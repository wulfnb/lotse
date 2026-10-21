//! UI state and serialized native debugger requests. Source IDs/offsets always come from Servo.
use serde_json::Value;
use std::{
    collections::{HashSet, VecDeque},
    time::Instant,
};
#[derive(Clone, PartialEq, Eq)]
pub enum Action {
    Frames,
    Scope(String),
    Eval {
        expression: String,
        console: bool,
        watch: Option<u64>,
    },
}
pub struct Request {
    pub owner: String,
    pub pipeline: servo_base::id::PipelineId,
    pub generation: u64,
    pub action: Action,
}
pub struct Call {
    pub request: Request,
    pub receive: Box<dyn FnMut() -> Option<Value>>,
    pub started: Instant,
    pub warned: bool,
}
#[derive(Default)]
pub struct Driver {
    pub queue: VecDeque<Request>,
    pub active: Option<Call>,
}
pub struct Watch {
    pub id: u64,
    pub expression: String,
    pub result: Option<Value>,
}
#[derive(Default)]
pub struct State {
    pub generation: u64,
    pub stack: Vec<String>,
    pub environment: Option<String>,
    pub watches: Vec<Watch>,
    pub watch_input: String,
    pub watch_next: u64,
    pub source_filter: String,
    pub search: String,
    pub hits: Vec<Hit>,
    pub search_truncated: bool,
    pub search_all: bool,
    pub goto: Option<u32>,
    pub ignored: HashSet<u32>,
    pub disabled: HashSet<(u32, u32)>,
    pub all_disabled: bool,
    pub sidebar: usize,
}
pub struct Hit {
    pub source: usize,
    pub line: u32,
    pub text: String,
}
pub fn search(
    sources: &[devtools_traits::SourceInfo],
    query: &str,
    current: Option<usize>,
) -> (Vec<Hit>, bool) {
    let mut hits = Vec::new();
    let mut budget = 16 * 1024 * 1024usize;
    let query = query.to_lowercase();
    if query.is_empty() {
        return (hits, false);
    }
    for (source, item) in sources.iter().enumerate() {
        if current.is_some_and(|i| i != source) {
            continue;
        }
        if let Some(text) = &item.content {
            for (line, text) in text.lines().enumerate() {
                if text.len() > budget || hits.len() >= 500 {
                    return (hits, true);
                }
                budget -= text.len();
                if text.to_lowercase().contains(&query) {
                    hits.push(Hit {
                        source,
                        line: (line + 1) as u32,
                        text: crate::console_model::bounded(text, 240),
                    });
                }
            }
        }
    }
    (hits, false)
}
