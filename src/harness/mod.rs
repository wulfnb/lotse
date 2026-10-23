//! Native integration scenarios driven from inside the running browser.
//! Built only with `--features test-harness`; release builds contain none of this.
pub mod capabilities;
pub mod coverage_ui;
pub mod download;
pub mod smoke;
pub mod theme;
pub mod toolbox;

use std::{cell::RefCell, rc::Rc, time::Instant};

/// At most one scenario runs per process (enforced by option parsing).
#[derive(Default)]
pub struct Harness {
    pub(crate) smoke: Option<Smoke>,
    pub(crate) download_test: Option<download::Test>,
    pub(crate) toolbox_test: Option<toolbox::Test>,
    pub(crate) theme_test: Option<theme::Test>,
    pub(crate) capabilities_test: Option<capabilities::Test>,
}

impl Harness {
    pub fn new(options: &wega::options::Options) -> Self {
        Self {
            smoke: options.smoke_test.then(|| Smoke {
                start: Instant::now(),
                stage_at: Instant::now(),
                stage: 0,
                result: Rc::new(RefCell::new(None)),
            }),
            download_test: options.download_test.then(download::Test::new),
            toolbox_test: options.toolbox_test.then(toolbox::Test::new),
            theme_test: options.theme_test.then(theme::Test::new),
            capabilities_test: options.capabilities_test.then(capabilities::Test::new),
        }
    }
    pub fn running(&self) -> bool {
        self.smoke.is_some()
            || self.download_test.is_some()
            || self.toolbox_test.is_some()
            || self.theme_test.is_some()
            || self.capabilities_test.is_some()
    }
}

pub struct Smoke {
    pub(crate) start: Instant,
    pub(crate) stage: u8,
    pub(crate) stage_at: Instant,
    pub(crate) result:
        Rc<RefCell<Option<Result<servo::JSValue, servo::JavaScriptEvaluationError>>>>,
}
