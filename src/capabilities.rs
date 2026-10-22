//! Browser policies and unprivileged internal documents. No page receives a native IPC bridge.
use servo::protocol_handler::*;
use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};

pub use servo_browser::model::Policy;
pub fn intercept(policy: &Policy, load: servo::WebResourceLoad) {
    let url = &load.request().url;
    if policy.offline || !policy.permits(url) {
        let mut response = servo::WebResourceResponse::new(url.clone());
        response.status_code = http::StatusCode::FORBIDDEN;
        response.headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("text/html; charset=utf-8"),
        );
        let body = if policy.offline {
            "Offline mode is enabled. Turn it off in Browser settings."
        } else {
            "This request was blocked by your browser host rules."
        };
        let mut stream = load.intercept(response);
        stream.send_body_data(
            format!(
                "<!doctype html><title>Request blocked</title><h1>Request blocked</h1><p>{body}</p>"
            )
            .into_bytes(),
        );
        stream.finish();
    }
}

pub enum Decision {
    Servo(servo::AllowOrDenyRequest),
    External(url::Url),
    Protocol(
        servo::AllowOrDenyRequest,
        servo::protocol_handler::ProtocolHandlerRegistration,
    ),
}
pub struct Prompt {
    pub title: String,
    pub detail: String,
    pub decision: Decision,
}
#[derive(Default)]
pub struct State {
    pub settings: bool,
    pub bluetooth: VecDeque<servo::BluetoothDeviceSelectionRequest>,
    pub traversals: std::collections::HashMap<servo::WebViewId, servo::TraversalId>,
    pub media: std::collections::HashMap<servo::WebViewId, crate::native_media::MediaInfo>,
    pub media_owner: Option<servo::WebViewId>,
    pub site_info: bool,
    pub clear_data: bool,
    pub delete_site: Option<String>,
    pub settings_section: usize,
    pub diagnostics: bool,
    pub prompts: VecDeque<Prompt>,
    pub sites: Vec<(String, String)>,
    pub cookies: Vec<String>,
    pub script: Option<std::rc::Rc<servo::UserScript>>,
    pub find_open: bool,
    pub find: String,
    pub script_source: String,
}

pub type Documents = Arc<RwLock<std::collections::HashMap<String, String>>>;
pub struct InternalPages(pub Documents);
impl ProtocolHandler for InternalPages {
    fn load<'a>(
        &'a self,
        request: &'a mut Request,
        _: &mut DoneChannel,
        _: &FetchContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send + 'a>> {
        let url = request.current_url();
        let page = url.host_str().unwrap_or("newtab");
        let body = self
            .0
            .read()
            .unwrap()
            .get(page)
            .cloned()
            .unwrap_or_else(|| "<h1>Page not found</h1>".into());
        let mut response = Response::new(url, ResourceFetchTiming::new(request.timing_type()));
        response.headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("text/html; charset=utf-8"),
        );
        response.headers.insert(http::header::CONTENT_SECURITY_POLICY, http::HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action https://www.google.com; frame-ancestors 'none'"));
        *response.body.lock() = ResponseBody::Done(body.into_bytes());
        Box::pin(std::future::ready(response))
    }
}
pub fn html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn theme_stylesheet(palette: crate::theme::Palette) -> String {
    let hex = |c: egui::Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
    format!(
        ":root{{--lotse-text:{};--lotse-background:{};--lotse-accent:{};--lotse-surface:{};--lotse-border:{};--lotse-selected:{}}}",
        hex(palette.text),
        hex(palette.inset),
        hex(palette.accent),
        hex(palette.surface),
        hex(palette.border),
        hex(palette.selected)
    )
}
pub fn document(title: &str, body: &str, palette: crate::theme::Palette) -> String {
    let variables = theme_stylesheet(palette);
    format!(
        "<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>{}</title><style id='lotse-theme'>{variables}</style><style>html{{font:16px system-ui;color:var(--lotse-text);background:var(--lotse-background)}}body{{max-width:850px;margin:48px auto;padding:24px}}a{{color:var(--lotse-accent)}}input,button{{font:inherit;padding:12px;border-radius:8px;max-width:100%;box-sizing:border-box;background:var(--lotse-surface);color:var(--lotse-text);border:1px solid var(--lotse-border)}}button{{background:var(--lotse-selected);cursor:pointer}}.search-controls{{display:flex;align-items:center;gap:8px;flex-wrap:wrap}}.search-controls input{{width:400px;min-width:0}}.search-controls input,.search-controls button{{height:48px}}nav{{display:flex;flex-wrap:wrap;gap:12px}}:focus-visible{{outline:3px solid var(--lotse-accent);outline-offset:4px}}li{{margin:14px 0}}p{{line-height:1.6}}</style></head><body><nav aria-label='Browser pages'><a href='lotse://newtab'>New tab</a> · <a href='lotse://history'>History</a> · <a href='lotse://bookmarks'>Bookmarks</a> · <a href='lotse://settings'>Settings</a></nav><main><h1>{}</h1>{body}</main></body></html>",
        html(title),
        html(title)
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn domain_rules_respect_boundaries_and_case() {
        let p = Policy {
            blocked_hosts: "tracker.example".into(),
            allowed_hosts: "example.com".into(),
            ..Default::default()
        };
        assert!(p.permits(&url::Url::parse("https://www.example.com/").unwrap()));
        assert!(!p.permits(&url::Url::parse("https://evil-example.com/").unwrap()));
        assert!(!p.permits(&url::Url::parse("https://tracker.example/").unwrap()));
        assert!(!p.permits(&url::Url::parse("https://example.com.evil.test/").unwrap()));
    }
    #[test]
    fn html_is_inert() {
        assert_eq!(html("<script>\"&'"), "&lt;script&gt;&quot;&amp;&#39;");
    }
}
