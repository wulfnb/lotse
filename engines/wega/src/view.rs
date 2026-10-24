use crate::{Limits, network::Network, script::ScriptHost};
use anyrender::ImageRenderer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{BaseDocument, DocumentConfig, NodeId, traversal::TreeTraverser};
use blitz_html::HtmlDocument;
use blitz_traits::{
    navigation::{NavigationOptions, NavigationProvider},
    net::{AbortController, AbortSignal, Bytes, NetHandler, NetProvider, Request},
    shell::{ColorScheme, Viewport},
};
use html5ever::LocalName;
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use url::Url;

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
type FetchResult = Result<(Url, Vec<u8>), String>;

struct Resources {
    network: Network,
    local: Option<std::path::PathBuf>,
    limit: usize,
    revision: Arc<AtomicU64>,
}
impl NetProvider for Resources {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let local = self.local.clone();
        let policy = self.network.policy.clone();
        let wake = self.network.wake.clone();
        let revision = self.revision.clone();
        let limit = self.limit;
        // HTTP GET subresources only; no access to local files from web pages.
        if request.method != blitz_traits::net::http::Method::GET {
            handler.bytes(request.url.to_string(), Bytes::new());
            return;
        }
        let handler = Arc::new(std::sync::Mutex::new(Some(handler)));
        let fallback = handler.clone();
        let failed_url = request.url.to_string();
        let failed_wake = self.network.wake.clone();
        let failed_revision = self.revision.clone();
        let task = move |client: &reqwest::blocking::Client| {
            if request.signal.as_ref().is_some_and(AbortSignal::aborted) {
                return;
            }
            let policy = policy
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let response = Network::fetch(client, &policy, &request.url, limit, local.as_deref());
            if request.signal.as_ref().is_some_and(AbortSignal::aborted) {
                return;
            }
            let (url, bytes) = match response {
                Ok((url, bytes)) => (url, bytes),
                Err(error) => {
                    eprintln!("Wega resource: {error}");
                    (request.url, vec![])
                }
            };
            if let Some(handler) = handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                handler.bytes(url.to_string(), Bytes::from(bytes));
            }
            revision.fetch_add(1, Ordering::Relaxed);
            wake();
        };
        if let Err(error) = self.network.spawn(task) {
            eprintln!("Wega resource: {error}");
            if let Some(handler) = fallback
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                handler.bytes(failed_url, Bytes::new());
            }
            failed_revision.fetch_add(1, Ordering::Relaxed);
            failed_wake();
        }
    }
}
struct Navigation(mpsc::Sender<Url>, Arc<dyn Fn() + Send + Sync>);
impl NavigationProvider for Navigation {
    fn navigate_to(&self, options: NavigationOptions) {
        if options.method == blitz_traits::net::http::Method::GET {
            let _ = self.0.send(options.url);
            (self.1)();
        }
    }
}
enum Script {
    Inline(String),
    External(Url),
}

pub struct View {
    document: Rc<RefCell<BaseDocument>>,
    script: Option<ScriptHost>,
    network: Network,
    limits: Limits,
    abort: Option<AbortController>,
    pending: Option<mpsc::Receiver<FetchResult>>,
    scripts: VecDeque<Script>,
    pending_script: Option<mpsc::Receiver<FetchResult>>,
    navigate: Rc<RefCell<Vec<Url>>>,
    navigation_rx: mpsc::Receiver<Url>,
    revision: Arc<AtomicU64>,
    last_revision: u64,
    renderer: Option<VelloCpuImageRenderer>,
    size: (u32, u32),
    started: Instant,
    dirty: bool,
    frozen: bool,
    ready_event_sent: bool,
    pub url: Url,
    pub loading: bool,
    pub notices: VecDeque<String>,
}
impl View {
    pub(crate) fn new(url: Url, network: Network, limits: Limits) -> Result<Self, String> {
        let document = HtmlDocument::from_html(
            "<!doctype html><title>Wega</title>",
            DocumentConfig::default(),
        )
        .into_inner();
        let (_tx, navigation_rx) = mpsc::channel();
        let mut view = Self {
            document: Rc::new(RefCell::new(document)),
            script: None,
            network,
            limits,
            abort: None,
            pending: None,
            scripts: VecDeque::new(),
            pending_script: None,
            navigate: Default::default(),
            navigation_rx,
            revision: Arc::new(AtomicU64::new(0)),
            last_revision: 0,
            renderer: None,
            size: (0, 0),
            started: Instant::now(),
            dirty: true,
            frozen: false,
            ready_event_sent: false,
            url: url.clone(),
            loading: false,
            notices: VecDeque::new(),
        };
        view.load(url)?;
        Ok(view)
    }
    fn fetch(&self, url: Url, limit: usize) -> Result<mpsc::Receiver<FetchResult>, String> {
        let (sender, receiver) = mpsc::channel();
        let wake = self.network.wake.clone();
        let policy = self.network.policy.clone();
        let local = if self.url.scheme() == "file" {
            self.url
                .to_file_path()
                .ok()
                .and_then(|p| p.canonicalize().ok())
                .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        } else {
            None
        };
        self.network.spawn(move |client| {
            let policy = policy
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let result = Network::fetch(client, &policy, &url, limit, local.as_deref());
            let _ = sender.send(result);
            wake();
        })?;
        Ok(receiver)
    }
    pub fn load(&mut self, url: Url) -> Result<(), String> {
        self.stop();
        self.url = url.clone();
        self.loading = true;
        self.ready_event_sent = false;
        if url.as_str() == "about:blank" {
            self.set_html("<!doctype html><title>New tab</title><body></body>")?;
        } else {
            self.pending = Some(self.fetch(url, self.limits.html_bytes)?);
        }
        Ok(())
    }
    pub fn stop(&mut self) {
        if let Some(abort) = self.abort.take() {
            abort.abort();
        }
        self.pending = None;
        self.pending_script = None;
        self.scripts.clear();
        self.script = None;
        self.loading = false;
    }
    pub fn set_html(&mut self, html: &str) -> Result<(), String> {
        if html.len() > self.limits.html_bytes {
            return Err("HTML document exceeds the size limit".into());
        }
        if let Some(abort) = self.abort.take() {
            abort.abort();
        }
        let abort = AbortController::default();
        let (sender, receiver) = mpsc::channel();
        self.navigation_rx = receiver;
        let local = if self.url.scheme() == "file" {
            self.url
                .to_file_path()
                .ok()
                .and_then(|p| p.canonicalize().ok())
                .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        } else {
            None
        };
        let config = DocumentConfig {
            base_url: Some(self.url.to_string()),
            abort_signal: Some(abort.signal.clone()),
            net_provider: Some(Arc::new(Resources {
                network: self.network.clone(),
                local,
                limit: self.limits.resource_bytes,
                revision: self.revision.clone(),
            })),
            navigation_provider: Some(Arc::new(Navigation(sender, self.network.wake.clone()))),
            ..Default::default()
        };
        let doc = HtmlDocument::from_html(html, config).into_inner();
        if TreeTraverser::new(&doc)
            .take(self.limits.dom_nodes + 1)
            .count()
            > self.limits.dom_nodes
        {
            return Err("DOM node limit exceeded; use Servo for this page".into());
        }
        let mut scripts = VecDeque::new();
        for id in doc
            .query_selector_all("script")
            .map_err(|_| "Unable to inspect scripts")?
            .iter()
            .take(64)
        {
            let node = doc.get_node(*id).unwrap();
            let element = node.element_data().unwrap();
            let kind = element.attr(LocalName::from("type")).unwrap_or_default();
            if kind == "module" {
                self.notices.push_back(
                    "JavaScript modules are not supported yet; use Servo for this page".into(),
                );
                continue;
            }
            if !kind.is_empty() && !matches!(kind, "text/javascript" | "application/javascript") {
                continue;
            }
            if let Some(source) = element.attr(LocalName::from("src")) {
                if let Ok(url) = self.url.join(source) {
                    scripts.push_back(Script::External(url));
                }
            } else {
                scripts.push_back(Script::Inline(node.text_content()));
            }
        }
        self.script = None;
        self.document = Rc::new(RefCell::new(doc));
        self.abort = Some(abort);
        self.scripts = scripts;
        self.script = Some(ScriptHost::new(
            self.document.clone(),
            &self.limits,
            self.navigate.clone(),
            html.len(),
        )?);
        self.dirty = true;
        self.ready_event_sent = false;
        Ok(())
    }
    pub fn poll(&mut self) {
        if self.frozen {
            return;
        }
        let result = self.pending.as_ref().and_then(|r| r.try_recv().ok());
        if let Some(result) = result {
            self.pending = None;
            match result {
                Ok((url, bytes)) => {
                    self.url = url;
                    let html = String::from_utf8_lossy(&bytes);
                    if let Err(error) = self.set_html(&html) {
                        self.load_error(error);
                    }
                }
                Err(error) => self.load_error(error),
            }
        }
        self.document.borrow_mut().handle_messages();
        let revision = self.revision.load(Ordering::Relaxed);
        if revision != self.last_revision {
            self.last_revision = revision;
            self.dirty = true;
        }
        let script_result = self.pending_script.as_ref().and_then(|r| r.try_recv().ok());
        if let Some(result) = script_result {
            self.pending_script = None;
            match result {
                Ok((_, bytes)) => self.run(&String::from_utf8_lossy(&bytes)),
                Err(error) => self.notice(error),
            }
        }
        if self.pending.is_none()
            && self.pending_script.is_none()
            && let Some(script) = self.scripts.pop_front()
        {
            match script {
                Script::Inline(source) => self.run(&source),
                Script::External(url) => match self.fetch(url, 1024 * 1024) {
                    Ok(rx) => self.pending_script = Some(rx),
                    Err(error) => self.notice(error),
                },
            }
        }
        if self.pending.is_none()
            && self.pending_script.is_none()
            && self.scripts.is_empty()
            && !self.ready_event_sent
        {
            self.ready_event_sent = true;
            self.loading = false;
            self.run("document.readyState='interactive';__wega_dispatch('DOMContentLoaded',document._id);document.readyState='complete';__wega_dispatch('load',document._id)");
        }
        if let Some(script) = &self.script {
            let due = script.next_timer().is_some_and(|t| t.is_zero());
            if due {
                let result = script.tick();
                if let Err(error) = result {
                    self.script = None;
                    self.scripts.clear();
                    self.notice(error);
                }
                self.dirty = true;
            }
        }
        while let Ok(url) = self.navigation_rx.try_recv() {
            self.navigate.borrow_mut().push(url);
        }
    }
    fn notice(&mut self, error: String) {
        if self.notices.len() >= 16 {
            self.notices.pop_front();
        }
        self.notices.push_back(error);
        self.dirty = true;
    }
    fn load_error(&mut self, error: String) {
        let escaped = error
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let _=self.set_html(&format!("<!doctype html><title>Page unavailable</title><body style='font:18px system-ui;padding:36px;background:white;color:#142b24'><h1>This page could not be loaded</h1><p>{escaped}</p><p>Retry, or choose Servo using the engine selector below.</p>"));
        self.loading = false;
        self.notice(error);
    }
    fn run(&mut self, source: &str) {
        if let Some(script) = &self.script {
            let result = script.run(source).and_then(|_| script.tick());
            if let Err(error) = result {
                self.script = None;
                self.scripts.clear();
                self.pending_script = None;
                self.notice(error);
            }
            self.dirty = true;
        }
    }
    pub fn evaluate(&mut self, source: &str) -> Result<String, String> {
        let result = self.script.as_ref().ok_or("No script context")?.run(source);
        self.dirty = true;
        result
    }
    pub fn evaluate_json(&mut self, source: &str) -> Result<String, String> {
        let result = self
            .script
            .as_ref()
            .ok_or("No script context")?
            .evaluate_json(source);
        self.dirty = true;
        result
    }
    pub fn title(&self) -> String {
        let doc = self.document.borrow();
        doc.query_selector("title")
            .ok()
            .flatten()
            .and_then(|id| doc.get_node(id))
            .map(|n| n.text_content())
            .unwrap_or_default()
    }
    pub fn next_navigation(&mut self) -> Option<Url> {
        let mut queue = self.navigate.borrow_mut();
        if queue.is_empty() {
            None
        } else {
            Some(queue.remove(0))
        }
    }
    pub fn next_wakeup(&self) -> Option<Duration> {
        if self.frozen {
            None
        } else if !self.scripts.is_empty() && self.pending_script.is_none() {
            Some(Duration::ZERO)
        } else {
            self.script.as_ref().and_then(ScriptHost::next_timer)
        }
    }
    pub fn heap_bytes(&self) -> usize {
        self.script.as_ref().map_or(0, ScriptHost::heap_bytes)
    }
    pub fn node_count(&self) -> usize {
        TreeTraverser::new(&self.document.borrow()).count()
    }
    pub fn needs_paint(&self) -> bool {
        self.dirty && !self.frozen
    }
    pub fn request_redraw(&mut self) {
        self.dirty = true;
    }
    pub fn freeze(&mut self, freeze: bool) {
        self.frozen = freeze;
        self.renderer = None;
        self.size = (0, 0);
        if !freeze {
            self.dirty = true;
        }
    }
    pub fn click(&mut self, x: f32, y: f32) {
        let id = self.document.borrow().element_from_point(x, y);
        if let Some(id) = id {
            let cancelled = self
                .script
                .as_ref()
                .map(|script| script.dispatch("click", id))
                .transpose();
            match cancelled {
                Err(error) => self.notice(error),
                Ok(Some(true)) => {}
                _ => {
                    let mut doc = self.document.borrow_mut();
                    if let Some(node) = doc.get_node(id) {
                        let mut event = blitz_traits::events::DomEvent::new(
                            id,
                            node.synthetic_click_event(Default::default()),
                        );
                        doc.handle_dom_event(&mut event, |_| {});
                    }
                }
            }
            self.dirty = true;
        }
    }
    pub fn scroll(&mut self, x: f64, y: f64) {
        let mut doc = self.document.borrow_mut();
        let p = doc.viewport_scroll();
        doc.set_viewport_scroll(blitz_dom::Point {
            x: p.x + x,
            y: p.y + y,
        });
        self.dirty = true;
    }
    pub fn paint(
        &mut self,
        width: u32,
        height: u32,
        scale: f32,
        zoom: f32,
        dark: bool,
    ) -> Result<Frame, String> {
        if width == 0
            || height == 0
            || width as usize * height as usize > self.limits.max_pixels
            || !scale.is_finite()
            || scale <= 0.0
            || !zoom.is_finite()
            || !(0.3..=5.0).contains(&zoom)
        {
            return Err("Invalid or oversized viewport".into());
        }
        let mut doc = self.document.borrow_mut();
        doc.set_viewport(Viewport {
            window_size: (width, height),
            hidpi_scale: scale,
            zoom,
            color_scheme: if dark {
                ColorScheme::Dark
            } else {
                ColorScheme::Light
            },
        });
        doc.resolve(self.started.elapsed().as_secs_f64());
        if self.size != (width, height) {
            self.renderer = Some(VelloCpuImageRenderer::new(width, height));
            self.size = (width, height);
        }
        let renderer = self.renderer.as_mut().unwrap();
        renderer.reset();
        let mut rgba = vec![];
        renderer.render_to_vec(
            |scene| {
                blitz_paint::paint_scene(
                    scene,
                    &mut doc,
                    (scale * zoom) as f64,
                    width,
                    height,
                    0,
                    0,
                )
            },
            &mut rgba,
        );
        self.dirty = false;
        Ok(Frame {
            width,
            height,
            rgba,
        })
    }
    pub fn text(&self, selector: &str) -> Option<String> {
        let doc = self.document.borrow();
        doc.query_selector(selector)
            .ok()
            .flatten()
            .and_then(|id| doc.get_node(id))
            .map(|n| n.text_content())
    }
    pub fn node_id(&self, selector: &str) -> Option<NodeId> {
        self.document
            .borrow()
            .query_selector(selector)
            .ok()
            .flatten()
    }
}
impl Drop for View {
    fn drop(&mut self) {
        if let Some(abort) = self.abort.take() {
            abort.abort();
        }
    }
}
