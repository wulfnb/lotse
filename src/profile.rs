use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use url::Url;

pub const HOME: &str = "https://www.google.com/";

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum TabLayout {
    Top,
    #[default]
    Side,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub visited_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedFavicon {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub policy: crate::model::Policy,
    pub favicons: std::collections::BTreeMap<String, SavedFavicon>,
    pub layout: TabLayout,
    pub devtools_dock: crate::model::Dock,
    pub theme: crate::model::Theme,
    pub sidebar_width: f32,
    pub urls: Vec<String>,
    pub active: usize,
    pub bookmarks: Vec<Bookmark>,
    pub history: Vec<Bookmark>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            policy: Default::default(),
            favicons: Default::default(),
            layout: TabLayout::Side,
            devtools_dock: Default::default(),
            theme: crate::model::Theme::default(),
            sidebar_width: 240.0,
            urls: vec![HOME.into()],
            active: 0,
            bookmarks: vec![
                Bookmark {
                    title: "Servo".into(),
                    url: "https://servo.org/".into(),
                    visited_at: 0,
                },
                Bookmark {
                    title: "Rust".into(),
                    url: "https://www.rust-lang.org/".into(),
                    visited_at: 0,
                },
            ],
            history: Vec::new(),
        }
    }
}

impl Profile {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("browser.json");
        match fs::read(&path) {
            Ok(bytes) => {
                let mut profile: Self = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
                for address in &mut profile.urls {
                    *address = canonical_internal_address(address);
                }
                profile.normalize();
                Ok(profile)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("Cannot read {}: {e}", path.display())),
        }
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        crate::storage::atomic_write(&dir.join("browser.json"), &bytes).map_err(|e| e.to_string())
    }

    /// Normalize legacy or manually edited settings before they reach the UI.
    pub fn normalize(&mut self) {
        if !self.sidebar_width.is_finite() {
            self.sidebar_width = 240.0;
        }
        self.sidebar_width = self.sidebar_width.clamp(150.0, 500.0);
        self.urls.retain(|url| resolve_address(url).is_ok());
        self.urls.truncate(100);
        if self.urls.is_empty() {
            self.urls.push(HOME.into());
        }
        self.active = self.active.min(self.urls.len() - 1);
        self.favicons.retain(|_, image| {
            image.width > 0
                && image.height > 0
                && image.width <= 64
                && image.height <= 64
                && image.rgba.len() == image.width * image.height * 4
        });
        while self.favicons.len() > 128 {
            self.favicons.pop_first();
        }
    }

    pub fn toggle_bookmark(&mut self, title: String, url: String) {
        if let Some(index) = self
            .bookmarks
            .iter()
            .position(|bookmark| bookmark.url == url)
        {
            self.bookmarks.remove(index);
        } else {
            self.bookmarks.push(Bookmark {
                title,
                url,
                visited_at: 0,
            });
        }
    }

    pub fn default_dir() -> PathBuf {
        directories::ProjectDirs::from("org", "ServoBrowser", "ServoBrowser")
            .map(|p| p.data_local_dir().to_owned())
            .unwrap_or_else(|| PathBuf::from(".servo-browser"))
    }
}

pub fn visit_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn history_date(visited_at: u64, now: chrono::DateTime<chrono::Local>) -> (String, String) {
    if visited_at == 0 {
        return ("Earlier visits".into(), String::new());
    }
    let Some(date) = i64::try_from(visited_at)
        .ok()
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|t| t.with_timezone(&chrono::Local))
    else {
        return ("Earlier visits".into(), String::new());
    };
    let day = date.date_naive();
    let today = now.date_naive();
    let group = if day == today {
        "Today".into()
    } else if today.pred_opt() == Some(day) {
        "Yesterday".into()
    } else {
        date.format("%d %b %Y").to_string()
    };
    (group, date.format("%H:%M").to_string())
}

// Retain legacy internal links while showing the current product scheme.
fn canonical_internal_address(input: &str) -> String {
    if let Ok(mut url) = Url::parse(input)
        && url.scheme() == "servo"
        && matches!(
            url.host_str(),
            Some("newtab" | "history" | "bookmarks" | "settings")
        )
    {
        let _ = url.set_scheme("lotse");
        return url.to_string();
    }
    input.into()
}

pub fn resolve_address(input: &str) -> Result<Url, String> {
    let input = input.trim();
    if input.is_empty() {
        return Url::parse(HOME).map_err(|e| e.to_string());
    }
    if input.starts_with('/') {
        return Url::from_file_path(input).map_err(|_| "Invalid local file path".into());
    }
    if input.contains("://") || input.starts_with("about:") || input.starts_with("data:") {
        let url = Url::parse(&canonical_internal_address(input)).map_err(|e| e.to_string())?;
        return match url.scheme() {
            "http" | "https" | "file" | "data" | "servo" | "lotse" => Ok(url),
            "about" if url.as_str() == "about:blank" => Ok(url),
            _ => Err(format!("Unsupported address scheme: {}", url.scheme())),
        };
    }
    if !input.chars().any(char::is_whitespace) {
        let local = input.starts_with("localhost")
            || input.starts_with("127.")
            || input.starts_with("[::1]");
        if local || input.contains('.') {
            return Url::parse(&format!(
                "{}://{input}",
                if local { "http" } else { "https" }
            ))
            .map_err(|e| e.to_string());
        }
    }
    let mut url = Url::parse("https://www.google.com/search").unwrap();
    url.query_pairs_mut().append_pair("q", input);
    Ok(url)
}

/// Move a tab while preserving the identity of the selected tab.
pub fn move_tab<T>(tabs: &mut Vec<T>, active: &mut usize, from: usize, to: usize) {
    if from >= tabs.len() || to >= tabs.len() || from == to {
        return;
    }
    let item = tabs.remove(from);
    tabs.insert(to, item);
    *active = if *active == from {
        to
    } else if from < *active && to >= *active {
        *active - 1
    } else if from > *active && to <= *active {
        *active + 1
    } else {
        *active
    };
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "profile_failure_tests.rs"]
mod failure_tests;
