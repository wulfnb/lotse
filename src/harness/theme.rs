//! Live switching and alpha-compositing checks in the real native window.
use crate::{browser::Browser, theme::Theme};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
pub struct Test {
    start: Instant,
    index: usize,
    since: Instant,
    started: bool,
    loaded: Option<Instant>,
    pub output: Option<PathBuf>,
    pub frame_checked: bool,
    final_result: Rc<RefCell<Option<bool>>>,
}
impl Test {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            index: 0,
            since: Instant::now(),
            started: false,
            loaded: None,
            output: None,
            frame_checked: false,
            final_result: Rc::new(RefCell::new(None)),
        }
    }
}
impl Browser {
    pub fn check_theme_test(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let Some(test) = &self.harness.theme_test else {
            return;
        };
        assert!(
            test.start.elapsed() < Duration::from_secs(90),
            "Theme test timed out at {}",
            test.index
        );
        if !test.started {
            if self.view().load_status() != servo::LoadStatus::Complete {
                return;
            }
            // Load completion precedes Servo's first painted frame; switching at once
            // exported an unpainted page for the first theme.
            let Some(loaded) = test.loaded else {
                self.harness.theme_test.as_mut().unwrap().loaded = Some(Instant::now());
                return;
            };
            if loaded.elapsed() < Duration::from_millis(1500) {
                return;
            }
            self.view()
                .evaluate_javascript("window.__themeSentinel=123", |_| {});
            self.harness.theme_test.as_mut().unwrap().started = true;
            self.select_test_theme();
            return;
        }
        if test.index < Theme::ALL.len()
            && test.frame_checked
            && test.since.elapsed() > Duration::from_millis(650)
        {
            let index = test.index + 1;
            self.harness.theme_test.as_mut().unwrap().index = index;
            if index < Theme::ALL.len() {
                self.select_test_theme();
            } else {
                let result = self
                    .harness
                    .theme_test
                    .as_ref()
                    .unwrap()
                    .final_result
                    .clone();
                self.view()
                    .evaluate_javascript("window.__themeSentinel", move |value| {
                        *result.borrow_mut() =
                            Some(matches!(value, Ok(servo::JSValue::Number(123.0))))
                    });
            }
            return;
        }
        if test.index == Theme::ALL.len()
            && let Some(preserved) = test.final_result.borrow_mut().take()
        {
            assert!(preserved, "Theme switching preserves JavaScript page state");
            println!(
                "THEME TEST PASSED: six live themes, native vibrancy state, framebuffer alpha, opaque web pages, rounded transparent corners, profile persistence and unchanged JavaScript state"
            );
            event_loop.exit();
        }
    }
    fn select_test_theme(&mut self) {
        let index = self.harness.theme_test.as_ref().unwrap().index;
        let theme = Theme::ALL[index];
        let ctx = self.gui.as_ref().unwrap().egui_ctx.clone();
        self.set_theme(&ctx, theme);
        self.save();
        assert_eq!(
            crate::profile::Profile::load(&self.profile_dir)
                .unwrap()
                .theme,
            theme
        );
        if cfg!(target_os = "macos") {
            assert!(self.native_theme.is_some(), "macOS backdrop was installed");
            assert_eq!(
                self.native_theme.as_ref().unwrap().glass_active(),
                theme.glass()
            );
        }
        let test = self.harness.theme_test.as_mut().unwrap();
        test.output = Some(self.profile_dir.join(format!("theme-{theme:?}.png")));
        test.frame_checked = false;
        test.since = Instant::now();
        self.window.request_redraw();
    }
}
