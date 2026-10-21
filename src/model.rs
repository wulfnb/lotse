//! Engine-independent persisted settings.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    Glass,
    GlassLight,
    GlassDark,
    Midnight,
    Paper,
    #[default]
    #[serde(alias = "Purple")]
    Emerald,
}
impl Theme {
    pub const ALL: [Self; 6] = [
        Self::Glass,
        Self::GlassLight,
        Self::GlassDark,
        Self::Midnight,
        Self::Paper,
        Self::Emerald,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Glass => "macOS Glass · Auto",
            Self::GlassLight => "Glass Light",
            Self::GlassDark => "Glass Dark",
            Self::Midnight => "Midnight",
            Self::Paper => "Paper",
            Self::Emerald => "Lotse · Emerald Glass",
        }
    }
    pub fn glass(self) -> bool {
        matches!(self, Self::Glass | Self::GlassLight | Self::GlassDark)
    }
    pub fn dark(self, system_dark: bool) -> bool {
        match self {
            Self::Glass => system_dark,
            Self::GlassLight | Self::Paper => false,
            _ => true,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Dock {
    #[default]
    Bottom,
    Left,
    Right,
    Window,
}
impl Dock {
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::Bottom, "Dock to bottom"),
        (Self::Right, "Dock to right"),
        (Self::Left, "Dock to left"),
        (Self::Window, "Separate window"),
    ];
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    pub blocked_hosts: String,
    pub allowed_hosts: String,
    pub offline: bool,
    pub allow_window_changes: bool,
    pub user_script: String,
}
impl Policy {
    pub fn permits(&self, url: &url::Url) -> bool {
        let host = url.host_str().unwrap_or_default().trim_end_matches('.');
        let matches = |list: &str| {
            list.split_whitespace().any(|rule| {
                let rule = rule.trim_start_matches("*.").trim_end_matches('.');
                if rule.is_empty() {
                    return false;
                }
                host.eq_ignore_ascii_case(rule)
                    || host
                        .to_ascii_lowercase()
                        .ends_with(&format!(".{}", rule.to_ascii_lowercase()))
            })
        };
        !matches(&self.blocked_hosts)
            && (self.allowed_hosts.trim().is_empty() || matches(&self.allowed_hosts))
    }
}

#[cfg(feature = "desktop")]
#[path = "theme_palette.rs"]
mod presentation;
#[cfg(feature = "desktop")]
pub use presentation::*;

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
