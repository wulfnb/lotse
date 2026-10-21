//! Bounded, chronological console records from Servo's native developer stream.
use serde_json::{Value, json};
use std::collections::{BTreeSet, VecDeque};
pub fn bounded(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.into();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}… [truncated]", &text[..end])
}
pub fn native_value(value: &devtools_traits::DebuggerValue) -> Value {
    if let devtools_traits::DebuggerValue::NumberValue(number) = value
        && !number.is_finite()
    {
        return json!({"NumberValue":if number.is_nan(){"NaN"}else if number.is_sign_positive(){"Infinity"}else{"-Infinity"}});
    }
    let mut result = serde_json::to_value(value).unwrap_or(Value::Null);
    if let devtools_traits::DebuggerValue::ObjectValue {
        preview: Some(preview),
        ..
    } = value
    {
        let target = &mut result["ObjectValue"]["preview"];
        if let Some(properties) = &preview.own_properties {
            for (index, property) in properties.iter().enumerate() {
                target["ownProperties"][index]["value"] = native_value(&property.value);
            }
        }
        if let Some(items) = &preview.items {
            target["items"] = json!(items.iter().map(native_value).collect::<Vec<_>>());
        }
        if let Some(entries) = &preview.entries {
            target["entries"] = json!(
                entries
                    .iter()
                    .map(|(k, v)| vec![native_value(k), native_value(v)])
                    .collect::<Vec<_>>()
            );
        }
    }
    result
}
pub fn value_text(value: &Value) -> String {
    if value == "VoidValue" {
        return "undefined".into();
    }
    for (name, kind) in [
        ("StringValue", 0),
        ("NumberValue", 1),
        ("BooleanValue", 1),
        ("NullValue", 2),
    ] {
        if let Some(v) = value.get(name) {
            return match kind {
                0 => v.as_str().unwrap_or("").into(),
                2 => {
                    if v.as_bool() == Some(true) {
                        "[uninitialized]".into()
                    } else {
                        "null".into()
                    }
                }
                _ => v.as_str().map(str::to_owned).unwrap_or_else(|| {
                    v.as_f64()
                        .map(|n| {
                            if n == 0.0 && n.is_sign_negative() {
                                "-0".into()
                            } else {
                                n.to_string()
                            }
                        })
                        .unwrap_or_else(|| v.to_string())
                }),
            };
        }
    }
    if let Some(v) = value.get("ObjectValue") {
        return format!(
            "{}{}",
            v["class"].as_str().unwrap_or("Object"),
            v["ownPropertyLength"]
                .as_u64()
                .map(|n| format!(" ({n} properties)"))
                .unwrap_or_default()
        );
    }
    bounded(&value.to_string(), 16384)
}
#[derive(Clone)]
pub struct Entry {
    pub id: u64,
    pub owner: String,
    pub level: String,
    pub category: String,
    pub text: String,
    pub time: u64,
    pub source: String,
    pub line: u32,
    pub column: u32,
    pub details: Value,
}
impl Entry {
    pub fn same_message(&self, other: &Self) -> bool {
        self.level == other.level
            && self.category == other.category
            && self.source == other.source
            && self.line == other.line
            && self.column == other.column
            && self.text == other.text
            && if self.details.get("arguments").is_some()
                && other.details.get("arguments").is_some()
            {
                self.details["arguments"] == other.details["arguments"]
                    && self.details["stacktrace"] == other.details["stacktrace"]
            } else {
                self.details == other.details
            }
    }
}
#[derive(Default)]
pub struct Log {
    pub entries: VecDeque<Entry>,
    next: u64,
    pub preserve: bool,
}
impl Log {
    pub fn push(&mut self, mut entry: Entry) {
        self.next += 1;
        entry.id = self.next;
        entry.text = bounded(&entry.text, 16384);
        entry.source = bounded(&entry.source, 8192);
        if entry.details.to_string().len() > 65536 {
            entry.details = json!({"note":"Structured preview exceeds 64 KiB"});
        }
        self.entries.push_back(entry);
        while self.entries.len() > 1000 {
            self.entries.pop_front();
        }
    }
    pub fn text(&mut self, owner: &str, level: &str, category: &str, text: String, details: Value) {
        self.push(Entry {
            id: 0,
            owner: owner.into(),
            level: level.into(),
            category: category.into(),
            text,
            time: devtools_traits::get_time_stamp(),
            source: String::new(),
            line: 0,
            column: 0,
            details,
        });
    }
    pub fn clear(&mut self, owner: &str) {
        self.entries.retain(|e| e.owner != owner);
    }
    pub fn navigation(&mut self, owner: &str) {
        if !self.preserve {
            self.clear(owner);
        }
    }
}
pub struct State {
    pub query: String,
    pub completions: Vec<String>,
    pub expanded: BTreeSet<u64>,
    pub export_open: bool,
    pub export_path: String,
    pub levels: BTreeSet<String>,
    pub categories: BTreeSet<String>,
    pub timestamps: bool,
    pub group_repeats: bool,
    pub follow: bool,
    pub multiline: bool,
    pub native_evaluation: bool,
    pub history: VecDeque<String>,
    pub history_index: Option<usize>,
    pub draft: String,
}
impl Default for State {
    fn default() -> Self {
        Self {
            query: String::new(),
            completions: Vec::new(),
            expanded: BTreeSet::new(),
            export_open: false,
            export_path: String::new(),
            levels: ["Error", "Warn", "Info", "Log", "Debug"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            categories: ["JS", "CSS", "Input", "Result"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            timestamps: false,
            group_repeats: true,
            follow: true,
            multiline: false,
            native_evaluation: false,
            history: VecDeque::new(),
            history_index: None,
            draft: String::new(),
        }
    }
}
impl State {
    pub fn matches(&self, e: &Entry) -> bool {
        self.levels.contains(&e.level)
            && self.categories.contains(&e.category)
            && self.query.split_whitespace().all(|term| {
                let (negative, term) = term
                    .strip_prefix('-')
                    .map(|s| (true, s))
                    .unwrap_or((false, term));
                let matched = format!("{} {} {}", e.text, e.source, e.category)
                    .to_lowercase()
                    .contains(&term.to_lowercase());
                matched != negative
            })
    }
    pub fn remember(&mut self, source: &str) {
        if self.history.back().is_none_or(|last| last != source) {
            self.history.push_back(source.into());
        }
        while self.history.len() > 100 {
            self.history.pop_front();
        }
        self.history_index = None;
        self.draft.clear();
    }
    pub fn history_move(&mut self, input: &mut String, back: bool) {
        if self.history.is_empty() {
            return;
        }
        let index = if back {
            match self.history_index {
                None => {
                    self.draft = input.clone();
                    self.history.len() - 1
                }
                Some(i) => i.saturating_sub(1),
            }
        } else {
            match self.history_index {
                Some(i) if i + 1 < self.history.len() => i + 1,
                _ => {
                    self.history_index = None;
                    *input = self.draft.clone();
                    return;
                }
            }
        };
        self.history_index = Some(index);
        *input = self.history[index].clone();
    }
}
#[cfg(test)]
#[path = "console_model_tests.rs"]
mod tests;
