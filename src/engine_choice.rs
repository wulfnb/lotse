//! Persisted identity is independent of either rendering backend.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineKind {
    #[default]
    Servo,
    Wega,
}
impl EngineKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Servo => "Servo",
            Self::Wega => "Wega (experimental)",
        }
    }
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "servo" => Ok(Self::Servo),
            "wega" => Ok(Self::Wega),
            _ => Err("Engine must be servo or wega".into()),
        }
    }
}
