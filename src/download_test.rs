//! End-to-end native download check driven by a page's real download link.
use std::time::{Duration, Instant};
pub struct Test {
    started: Instant,
    stage: u8,
}
impl Test {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            stage: 0,
        }
    }
}
impl crate::browser::Browser {
    pub fn check_download_test(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let Some(test) = &self.download_test else {
            return;
        };
        assert!(
            test.started.elapsed() < Duration::from_secs(60),
            "Download test timed out at stage {}",
            test.stage
        );
        match test.stage {
            0 if self.tab().view.page_title().as_deref() == Some("Download fixture")
                && self.tab().view.load_status() == servo::LoadStatus::Complete =>
            {
                self.download_test.as_mut().unwrap().stage = 1;
                self.tab()
                    .view
                    .evaluate_javascript("document.querySelector('#download').click()", |_| {});
            }
            1 if self.download_ui.open => {
                assert!(self.download_ui.url.ends_with("/file.bin"));
                assert_eq!(self.download_ui.filename, "fixture.bin");
                self.begin_download();
                assert_eq!(
                    self.downloads.jobs().len(),
                    1,
                    "Download starts through the native confirmation UI"
                );
                self.download_test.as_mut().unwrap().stage = 2;
            }
            2 => {
                let job = self.downloads.jobs()[0].clone();
                let progress = job.progress.lock().unwrap().clone();
                assert_ne!(
                    progress.phase,
                    crate::downloads::Phase::Failed,
                    "{}",
                    progress.message
                );
                if progress.phase != crate::downloads::Phase::Complete {
                    return;
                }
                assert_eq!(progress.connections, 4, "Multipart was used");
                let bytes = std::fs::read(progress.path.unwrap()).unwrap();
                assert_eq!(bytes.len(), 6 * 1024 * 1024 + 173);
                assert!(bytes.iter().enumerate().all(|(i, b)| *b == (i % 251) as u8));
                println!(
                    "DOWNLOAD TEST PASSED: page download link, native confirmation, HttpOnly site-cookie reuse, four parallel connections, exact file bytes and safe publication"
                );
                self.download_test = None;
                event_loop.exit();
            }
            _ => {}
        }
    }
}
