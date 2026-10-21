//! Lotse core and application services. The default-free build has no Servo or GPU dependency.
#![forbid(unsafe_code)]
pub mod downloads;
pub mod model;
pub mod options;
pub mod profile;
pub mod session;
pub mod storage;
#[cfg(feature = "desktop")]
pub mod theme {
    pub use crate::model::{Palette, install, palette};
}
pub mod application;
pub mod deadline;
pub mod navigation;
pub mod permissions;
