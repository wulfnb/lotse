//! Servo adapter for the engine-independent navigation port.
use servo_browser::navigation::NavigationPort;
pub(crate) struct Navigation<'a> {
    pub view: &'a servo::WebView,
    pub traversals: &'a mut std::collections::HashMap<servo::WebViewId, servo::TraversalId>,
}
impl NavigationPort for Navigation<'_> {
    fn load(&mut self, url: url::Url) {
        self.view.load(url);
    }
    fn reload(&mut self) {
        self.view.reload();
    }
    fn traverse(&mut self, forward: bool) -> bool {
        if (forward && !self.view.can_go_forward()) || (!forward && !self.view.can_go_back()) {
            return false;
        }
        let id = if forward {
            self.view.go_forward(1)
        } else {
            self.view.go_back(1)
        };
        self.traversals.insert(self.view.id(), id);
        true
    }
    fn zoom(&self) -> f32 {
        self.view.page_zoom()
    }
    fn set_zoom(&mut self, value: f32) {
        self.view.set_page_zoom(value);
    }
}
