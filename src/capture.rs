//! Deterministic exports of the browser's own GPU frames for rendering diagnostics.
use servo::RenderingContext;
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

pub struct Capture {
    pub output: PathBuf,
    start: Instant,
    loaded: Option<Instant>,
    requested_report: bool,
    report: Rc<RefCell<Option<String>>>,
}

impl Capture {
    pub fn new(output: PathBuf) -> Self {
        Self {
            output,
            start: Instant::now(),
            loaded: None,
            requested_report: false,
            report: Rc::new(RefCell::new(None)),
        }
    }
    pub fn ready_for_frame(&self) -> bool {
        self.loaded
            .is_some_and(|at| at.elapsed() > Duration::from_secs(2))
    }
}

impl crate::browser::Browser {
    pub fn check_capture(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let complete = self.tab().view.load_status() == servo::LoadStatus::Complete;
        let view = self.tab().view.clone();
        let Some(capture) = &mut self.capture else {
            return;
        };
        assert!(
            capture.start.elapsed() < Duration::from_secs(60),
            "Page capture timed out"
        );
        if complete && capture.loaded.is_none() {
            capture.loaded = Some(Instant::now());
        }
        if capture.ready_for_frame() && !capture.requested_report {
            capture.requested_report = true;
            let report = capture.report.clone();
            view.evaluate_javascript(
                "JSON.stringify({title:document.title,url:location.href,textGradients:Array.from(document.querySelectorAll('.bg-clip-text')).map(e=>({text:e.textContent,color:getComputedStyle(e).color,background:getComputedStyle(e).backgroundImage}))},null,2)",
                move |result| *report.borrow_mut() = Some(match result { Ok(servo::JSValue::String(text)) => text, other => format!("{other:?}") }),
            );
        }
        let Some(loaded) = capture.loaded else {
            return;
        };
        if loaded.elapsed() < Duration::from_secs(3) {
            return;
        }
        let Some(report) = capture.report.borrow_mut().take() else {
            return;
        };
        let output = capture.output.clone();
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).expect("Capture directory");
        }
        let surface = &self.tab().surface;
        surface.make_current().expect("Capture GPU context");
        self.tab().view.paint();
        let size = surface.size();
        let rect = servo::DeviceIntRect::from_size(servo::DeviceIntSize::new(
            size.width as i32,
            size.height as i32,
        ));
        surface
            .read_to_image(rect)
            .expect("Capture pixels")
            .save(&output)
            .expect("Save page capture");
        std::fs::write(output.with_extension("json"), report).expect("Save capture report");
        println!("CAPTURE PASSED: {}", output.display());
        self.capture = None;
        event_loop.exit();
    }
}
