//! Desktop integration test. Exercises the same native pointer-routing path as user input.
use crate::{browser::Browser, profile};
use servo::{InputEvent, JSValue, LoadStatus};
use std::time::{Duration, Instant};
use winit::{
    event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
    event_loop::ActiveEventLoop,
};

impl Browser {
    fn smoke_stage(&mut self, stage: u8) {
        let smoke = self.smoke.as_mut().unwrap();
        smoke.stage = stage;
        smoke.stage_at = Instant::now();
    }

    fn smoke_evaluate(&mut self, script: &str, next_stage: u8) {
        let result = self.smoke.as_ref().unwrap().result.clone();
        self.tab()
            .view
            .evaluate_javascript(script, move |value| *result.borrow_mut() = Some(value));
        self.smoke_stage(next_stage);
    }

    fn smoke_result(&self) -> Option<String> {
        let result = self.smoke.as_ref()?.result.borrow_mut().take()?;
        match result {
            Ok(JSValue::String(text)) => Some(text),
            result => {
                eprintln!("SMOKE TEST FAILED: unexpected JavaScript result: {result:?}");
                std::process::exit(1);
            }
        }
    }

    pub(crate) fn smoke_click(&mut self, x: f64, y: f64) {
        let scale = self.window.scale_factor();
        let position = winit::dpi::PhysicalPosition::new(
            (self.chrome.page_rect.min.x as f64 + x) * scale,
            (self.chrome.page_rect.min.y as f64 + y) * scale,
        );
        self.handle_event(WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position,
        });
        for state in [ElementState::Pressed, ElementState::Released] {
            self.handle_event(WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state,
                button: MouseButton::Left,
            });
        }
    }

    fn smoke_drag_tab(&mut self, from: usize, to: usize) {
        self.handle_event(WindowEvent::RedrawRequested);
        let source_id = self.chrome.tab_rects[from].0;
        let start = self.chrome.tab_rects[from].1.center();
        let end = self.chrome.tab_rects[to].1.center();
        let scale = self.window.scale_factor();
        let move_to = |browser: &mut Browser, point: egui::Pos2| {
            browser.handle_event(WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: winit::dpi::PhysicalPosition::new(
                    point.x as f64 * scale,
                    point.y as f64 * scale,
                ),
            });
            browser.handle_event(WindowEvent::RedrawRequested);
        };
        move_to(self, start);
        self.handle_event(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Left,
        });
        self.handle_event(WindowEvent::RedrawRequested);
        move_to(self, start + egui::vec2(16.0, 0.0));
        move_to(self, end);
        self.handle_event(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Released,
            button: MouseButton::Left,
        });
        self.handle_event(WindowEvent::RedrawRequested);
        assert_eq!(
            self.session().tabs()[to].view.id(),
            source_id,
            "Dragging must reorder the actual UI tabs"
        );
    }

    pub fn check_smoke_test(&mut self, event_loop: &ActiveEventLoop) {
        self.check_download_test(event_loop);
        self.check_toolbox_test(event_loop);
        self.check_theme_test(event_loop);
        self.check_capabilities_test(event_loop);
        let Some(smoke) = &self.smoke else {
            return;
        };
        if smoke.start.elapsed() > Duration::from_secs(90) {
            eprintln!(
                "SMOKE TEST FAILED: timeout at stage {}, url={}, title={:?}",
                smoke.stage,
                self.tab().url(),
                self.tab().view.page_title()
            );
            std::process::exit(1);
        }
        let stage = smoke.stage;
        let complete = self.tab().view.load_status() == LoadStatus::Complete;
        let title = self.tab().view.page_title().unwrap_or_default();
        match stage {
            0 if complete
                && title == "Servo interaction fixture"
                && self.chrome.page_rect.is_positive() =>
            {
                self.smoke_evaluate("JSON.stringify(['counter','input'].map(id => { const r=document.getElementById(id).getBoundingClientRect(); return [r.x+r.width/2,r.y+r.height/2]; }))", 1);
            }
            1 => {
                let Some(text) = self.smoke_result() else {
                    return;
                };
                let coords: Vec<[f64; 2]> =
                    serde_json::from_str(&text).expect("Fixture control coordinates");
                self.smoke_stage(2);
                self.smoke_click(coords[0][0], coords[0][1]);
                self.smoke_click(coords[1][0], coords[1][1]);
                for character in "Servo typed".chars() {
                    for state in [servo::KeyState::Down, servo::KeyState::Up] {
                        self.tab().view.notify_input_event(InputEvent::Keyboard(
                            servo::KeyboardEvent::from_state_and_key(
                                state,
                                servo::Key::Character(character.to_string()),
                            ),
                        ));
                    }
                }
                self.handle_event(WindowEvent::MouseWheel {
                    device_id: DeviceId::dummy(),
                    delta: MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
                        0.0, -500.0,
                    )),
                    phase: TouchPhase::Moved,
                });
                self.toggle_layout();
                self.toggle_layout();
                self.window.request_redraw();
            }
            2 if smoke.stage_at.elapsed() > Duration::from_millis(600) => {
                self.smoke_evaluate("JSON.stringify({button:document.querySelector('#counter').textContent, input:document.querySelector('#input').value, scrolled:window.scrollY>0, gradientColor:getComputedStyle(document.querySelector('#gradient-probe')).color, brandColor:getComputedStyle(document.querySelector('#brand-gradient')).color, gradientBackground:getComputedStyle(document.querySelector('#gradient-probe')).backgroundImage, normalBackground:getComputedStyle(document.querySelector('#normal-gradient')).backgroundImage})", 3);
            }
            3 => {
                let Some(text) = self.smoke_result() else {
                    return;
                };
                let data: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(
                    data["gradientBackground"], "none",
                    "Unsupported text gradients must not cover their text"
                );
                assert_eq!(
                    data["brandColor"], "rgb(0, 128, 94)",
                    "Text fallback must preserve the gradient brand color when provided"
                );
                assert_eq!(
                    data["gradientColor"], "rgb(238, 238, 238)",
                    "Fallback must preserve the parent text color"
                );
                assert_ne!(
                    data["normalBackground"], "none",
                    "Ordinary gradient backgrounds must remain unchanged"
                );
                if data["button"] != "Clicked 1"
                    || data["input"] != "Servo typed"
                    || data["scrolled"] != true
                {
                    eprintln!("SMOKE TEST FAILED: pointer/keyboard/scroll: {text}");
                    std::process::exit(1);
                }
                assert!(
                    self.console
                        .iter()
                        .any(|(_, text)| text.contains("Live Servo console")),
                    "Real console messages must reach the browser"
                );
                // Click the real toolbar control and verify document state resets.
                let position = self.chrome.reload_rect.center();
                let scale = self.window.scale_factor();
                self.handle_event(WindowEvent::CursorMoved {
                    device_id: DeviceId::dummy(),
                    position: winit::dpi::PhysicalPosition::new(
                        position.x as f64 * scale,
                        position.y as f64 * scale,
                    ),
                });
                self.handle_event(WindowEvent::RedrawRequested);
                for state in [ElementState::Pressed, ElementState::Released] {
                    self.handle_event(WindowEvent::MouseInput {
                        device_id: DeviceId::dummy(),
                        state,
                        button: MouseButton::Left,
                    });
                    self.handle_event(WindowEvent::RedrawRequested);
                }
                self.smoke_stage(11);
            }
            11 if complete && smoke.stage_at.elapsed() > Duration::from_millis(650) => {
                self.smoke_evaluate("JSON.stringify({count:window.count,input:document.querySelector('#input').value})",12);
            }
            12 => {
                let Some(text) = self.smoke_result() else {
                    return;
                };
                let data: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(
                    data["count"], 0,
                    "The refresh toolbar button must reload the document"
                );
                assert_eq!(data["input"], "", "Refresh must reset page input state");
                let url = self.tab().view.url().unwrap().join("second.html").unwrap();
                self.navigate(url.as_str());
                self.smoke_stage(4);
            }
            // History updates arrive separately from the document's load/title
            // callbacks. Await both, within the suite's existing 90s timeout.
            4 if complete && title == "Second Servo document" && self.tab().view.can_go_back() => {
                assert!(self.tab().view.can_go_back());
                self.traverse(false);
                self.smoke_stage(5);
            }
            5 if complete
                && title == "Servo interaction fixture"
                && self.tab().view.can_go_forward() =>
            {
                assert!(self.tab().view.can_go_forward());
                self.traverse(true);
                self.smoke_stage(6);
            }
            6 if complete && title == "Second Servo document" => {
                self.tab()
                    .view
                    .evaluate_javascript("window.open('interaction.html')", |_| {});
                self.smoke_stage(7);
            }
            7 if self.session().tabs().len() == 2
                && complete
                && title == "Servo interaction fixture" =>
            {
                let active_id = self.tab().view.id();
                self.smoke_drag_tab(1, 0);
                assert_eq!(self.tab().view.id(), active_id);
                self.toggle_layout();
                self.handle_event(WindowEvent::RedrawRequested);
                let center = self.chrome.tab_rects[0].1.center().y;
                assert!(
                    self.chrome
                        .tab_rects
                        .iter()
                        .all(|(_, rect)| (rect.center().y - center).abs() < 1.0),
                    "Active and inactive top tabs share a baseline: {:?}",
                    self.chrome.tab_rects
                );
                self.smoke_drag_tab(0, 1);
                assert_eq!(self.tab().view.id(), active_id);
                self.close_tab(0);
                self.toggle_layout();
                assert_eq!(self.tab().view.id(), active_id);
                self.bookmark();
                self.zoom(1.25);
                assert!((self.tab().view.page_zoom() - 1.25).abs() < 0.001);
                self.reset_zoom();
                assert!((self.tab().view.page_zoom() - 1.0).abs() < 0.001);
                self.save();
                let restored = profile::Profile::load(&self.profile_dir).unwrap();
                assert_eq!(restored.urls, vec![self.tab().url()]);
                assert_eq!(restored.layout, self.profile().layout);
                assert!(restored.bookmarks.iter().any(|b| b.url == self.tab().url()));
                assert!(restored.history.iter().all(|h| h.visited_at > 0));
                self.smoke_stage(8);
                self.window.request_redraw();
            }
            8 if smoke.stage_at.elapsed() > Duration::from_millis(600) => {
                let surface = &self.tab().surface;
                surface.make_current().expect("Smoke GPU context");
                self.tab().view.paint();
                use servo::RenderingContext;
                let size = surface.size();
                let rect = servo::DeviceIntRect::from_size(servo::DeviceIntSize::new(
                    size.width as i32,
                    size.height as i32,
                ));
                let image = surface
                    .read_to_image(rect)
                    .expect("Smoke test must produce pixels");
                assert!(
                    image.pixels().any(|p| p[0] > 150 && p[1] > 150),
                    "Rendered page must contain visible text"
                );
                let output = self.profile_dir.join("smoke-page.png");
                image.save(&output).expect("Cannot save smoke screenshot");
                self.chrome.show_history = true;
                self.chrome.show_bookmarks = false;
                self.smoke_stage(9);
                self.window.request_redraw();
            }
            9 if smoke.stage_at.elapsed() > Duration::from_millis(650) => {
                assert!(self.chrome.library_rect.is_positive());
                assert!(
                    (self.chrome.library_rect.min.x - self.chrome.page_rect.max.x - 16.0).abs()
                        < 2.0,
                    "History card must have a 16-point gap beside the inset page"
                );
                self.chrome.show_history = false;
                self.chrome.show_bookmarks = true;
                self.smoke_stage(10);
                self.window.request_redraw();
            }
            10 if smoke.stage_at.elapsed() > Duration::from_millis(650) => {
                assert!(self.chrome.library_rect.is_positive());
                assert!(
                    (self.chrome.library_rect.min.x - self.chrome.page_rect.max.x - 16.0).abs()
                        < 2.0,
                    "Bookmarks card must have a 16-point gap beside the inset page"
                );
                println!(
                    "SMOKE TEST PASSED: Servo HTML/CSS/JS, native mouse routing, typing, scrolling, back/forward, popup tabs, top/side layouts, reorder, close, bookmarks, profile round-trip, console, toolbar reload, readable gradient text, unchanged regular gradients, aligned library panels, GPU pixels"
                );
                self.smoke = None;
                event_loop.exit();
            }
            _ => {}
        }
    }
}
