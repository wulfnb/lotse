//! Live integration checks, driven by the app's event loop and real HTTP fixture.
use crate::{
    browser::Browser,
    network,
    toolbox::{Tool, pretty},
};
use std::time::{Duration, Instant};
pub struct Test {
    start: Instant,
    stage: usize,
    at: Instant,
    pub capture: Option<String>,
    height: f32,
    page_height: f32,
}
impl Test {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            stage: 0,
            at: Instant::now(),
            capture: None,
            height: 0.0,
            page_height: 0.0,
        }
    }
}
const PROBES: [Tool; 7] = [
    Tool::Inspector,
    Tool::StyleEditor,
    Tool::Performance,
    Tool::Memory,
    Tool::Storage,
    Tool::Accessibility,
    Tool::Application,
];
impl Browser {
    fn assert_toolbox_side_dock(&self, left: bool) {
        let screen = self.gui.as_ref().unwrap().egui_ctx.content_rect();
        let panel = self.tools.panel_rect;
        let page = self.chrome.page_rect;
        let valid = if crate::ux::compact(screen.width()) {
            panel.min.y > page.max.y
        } else if left {
            panel.max.x < page.min.x
        } else {
            panel.min.x > page.max.x
        };
        assert!(
            valid,
            "Side dock must fall back to bottom on compact windows: left={left}, screen={screen:?}, page={page:?}, panel={panel:?}"
        );
    }

    fn advanced_location(&self, owner: &str) -> (u32, u32) {
        let source = self.tools.debug[owner]
            .sources
            .iter()
            .find(|s| s.url.as_str().ends_with("advanced-tools.js"))
            .expect("Captured advanced fixture source");
        let line = source
            .content
            .as_ref()
            .unwrap()
            .lines()
            .position(|s| s.contains("window.advancedCalls +="))
            .unwrap()
            + 1;
        (source.spidermonkey_id, line as u32)
    }
    fn tool_stage(&mut self, stage: usize) {
        let test = self.harness.toolbox_test.as_mut().unwrap();
        println!("TOOLBOX stage {stage}");
        test.stage = stage;
        test.at = Instant::now();
        test.capture = None;
    }
    pub fn check_toolbox_test(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let Some(test) = &self.harness.toolbox_test else {
            return;
        };
        assert!(
            test.start.elapsed() < Duration::from_secs(240),
            "Toolbox test timeout at stage {}",
            test.stage
        );
        let stage = test.stage;
        let elapsed = test.at.elapsed();
        let view = self.view().clone();
        let owner = network::owner(view.id());
        if stage == 0 {
            if view.page_title().as_deref() != Some("Developer toolbox fixture")
                || self
                    .tools
                    .network
                    .rows
                    .iter()
                    .filter(|r| r.owner == owner && r.status.is_some())
                    .count()
                    < 6
                || !self
                    .tools
                    .network
                    .rows
                    .iter()
                    .any(|r| r.owner == owner && r.status == Some(404))
                || !self.tools.network.rows.iter().any(|r| {
                    r.owner == owner && r.status == Some(200) && r.url.contains("redirected=1")
                })
            {
                return;
            }
            assert!(self.tools.debug.contains_key(&owner), "Real script context");
            let rows = self.tools.network.visible(&owner);
            assert!(
                rows.iter().any(|r| r.status == Some(404)),
                "404 response captured"
            );
            let post = rows
                .iter()
                .find(|r| r.method == "POST")
                .expect("POST request");
            assert_eq!(post.status, Some(201));
            assert!(post.request_body.text().unwrap_or("").contains("Servo"));
            assert!(post.response_body.text().unwrap_or("").contains("received"));
            assert!(
                post.response_headers
                    .iter()
                    .any(|(k, v)| k == "x-servo-fixture" && v == "network-test")
            );
            assert!(
                rows.iter().any(|r| r.url.contains("redirected=1")),
                "Redirect destination captured"
            );
            assert!(
                rows.iter().any(|r| r.url.ends_with("toolbox.html")),
                "Main document retained after navigation"
            );
            // Page height before any toolbox docks. The OS may clamp the requested
            // window to a small display (1024x643 on macOS CI runners), so later
            // checks compare against this instead of a fixed pixel count.
            self.harness.toolbox_test.as_mut().unwrap().page_height =
                self.chrome.page_rect.height();
            self.tools.network.selected = Some(post.uid);
            self.tools.detail_tab = 2;
            self.tools.active = Tool::Network;
            self.chrome.show_console = true;
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-network.png".into());
            self.tool_stage(1);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-network.png".into());
            self.window.request_redraw();
            return;
        }
        if stage == 1 && elapsed > Duration::from_millis(700) {
            assert!(
                (self.tools.panel_rect.min.y - self.chrome.page_rect.max.y - 16.0).abs() < 2.0,
                "Toolbox docks under the page"
            );
            std::fs::write(
                self.profile_dir.join("network.har"),
                pretty(&self.tools.network.har(&owner)),
            )
            .unwrap();
            self.harness.toolbox_test.as_mut().unwrap().height = self.tools.panel_rect.height();
            self.tools.active = PROBES[0];
            self.tools.refresh(&view, &self.servo);
            self.tool_stage(2);
            return;
        }
        if (2..9).contains(&stage) {
            let tool = PROBES[stage - 2];
            let Some(snapshot) = self
                .tools
                .pages
                .get(&owner)
                .and_then(|p| p.snapshots.get(&tool))
            else {
                return;
            };
            if snapshot.pending || snapshot.updated.is_none() {
                return;
            }
            if tool == Tool::Storage && self.tools.pages[&owner].cookies.pending() {
                return;
            }
            assert!(snapshot.error.is_none(), "{tool:?}: {:?}", snapshot.error);
            let data = &snapshot.data;
            match tool {
                Tool::Inspector => assert!(
                    data["nodes"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|n| n["id"] == "heading"))
                ),
                Tool::StyleEditor => assert!(
                    self.tools.pages[&owner]
                        .style_sheets
                        .as_array()
                        .is_some_and(|a| !a.is_empty())
                ),
                Tool::Performance => assert!(data["resources"].is_array()),
                Tool::Memory => assert!(data["results"].as_array().is_some_and(|a| !a.is_empty())),
                Tool::Storage => {
                    assert!(
                        data["local"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|n| n["key"] == "fixture"))
                    );
                    let cookies = &self.tools.pages[&owner].cookies;
                    assert!(cookies.loaded, "Engine cookies must load asynchronously");
                    assert!(cookies.error.is_none(), "{:?}", cookies.error);
                    assert!(cookies.rows.iter().any(|row| row.name == "wega_http_only"
                        && row.value == "native-only-fixture"
                        && row.http_only
                        && row.path == "/"
                        && row.same_site == "Strict"));
                    assert!(
                        cookies
                            .rows
                            .iter()
                            .any(|row| row.name == "wega_visible" && !row.http_only)
                    );
                    assert!(
                        !cookies.rows.iter().any(|row| row.name == "wega_wrong_path"),
                        "Only URL-matching cookies should be shown"
                    );
                    let visible = data["cookies"].as_str().unwrap_or_default();
                    assert!(visible.contains("wega_visible="));
                    assert!(
                        !visible.contains("wega_http_only="),
                        "Native HttpOnly values must never enter the page probe"
                    );
                }
                Tool::Accessibility => assert_eq!(data["language"], "en"),
                Tool::Application => assert_eq!(data["manifest"]["name"], "Servo toolbox fixture"),
                _ => {}
            }
            std::fs::write(
                self.profile_dir.join(format!("{:?}.json", tool)),
                pretty(data),
            )
            .unwrap();
            if stage < 8 {
                self.tools.active = PROBES[stage - 1];
                self.tools.refresh(&view, &self.servo);
                self.tool_stage(stage + 1);
            } else {
                assert!(
                    !self.tools.debug[&owner].sources.is_empty(),
                    "Real script sources"
                );
                self.tools.active = Tool::Console;
                self.tools.page(&view).console_input = "1+2".into();
                self.tools.console(&view);
                self.tool_stage(9);
            }
            return;
        }
        if stage == 9 && elapsed > Duration::from_millis(400) && !self.tools.pending() {
            assert!(
                self.tools.pages[&owner]
                    .console_results
                    .iter()
                    .any(|v| v == "\"3\""),
                "Console evaluates real JS: {:?}",
                self.tools.pages[&owner].console_results
            );
            self.tools.stylesheet(&view, 0);
            self.tool_stage(10);
            return;
        }
        if stage == 10 && !self.tools.pending() {
            assert!(
                self.tools.pages[&owner].css.contains("color"),
                "Native stylesheet source"
            );
            self.tools.active = Tool::Inspector;
            self.tools.page(&view).selector = "#heading".into();
            self.tools.refresh(&view, &self.servo);
            self.tool_stage(11);
            return;
        }
        if stage == 11 && !self.tools.pending() {
            assert!(self.tools.pages[&owner].html.contains("A real Servo page"));
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-inspector.png".into());
            self.tool_stage(12);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-inspector.png".into());
            self.window.request_redraw();
            return;
        }
        if stage == 12 && elapsed > Duration::from_millis(700) {
            self.tools.network.clear(&owner);
            assert!(self.tools.network.visible(&owner).is_empty());
            self.tools
                .command(&owner, devtools_traits::DevtoolScriptControlMsg::Interrupt);
            view.evaluate_javascript(
                "setTimeout(function toolboxPause(){window.__debuggerResumed=true;},100)",
                |_| {},
            );
            self.tool_stage(13);
            return;
        }
        if stage == 13 && self.tools.debug.get(&owner).is_some_and(|d| d.paused) {
            assert!(self.tools.debug[&owner].frame.is_some());
            self.tools.resume(&owner, None);
            self.tools.active = Tool::Console;
            self.tools.page(&view).console_input =
                "new Promise(resolve=>setTimeout(()=>resolve(window.__debuggerResumed),200))"
                    .into();
            self.tools.console(&view);
            self.tool_stage(14);
            return;
        }
        if stage == 14 && elapsed > Duration::from_millis(300) && !self.tools.pending() {
            assert!(
                self.tools.pages[&owner]
                    .console_results
                    .iter()
                    .any(|v| v == "\"true\""),
                "Debugger resumed real JavaScript"
            );
            assert!(
                !self.tools.pages[&owner].ax.is_empty(),
                "Actual engine accessibility updates"
            );
            let source = self.tools.debug[&owner]
                .sources
                .iter()
                .find(|s| s.url.as_str().ends_with("toolbox.js"))
                .expect("External source")
                .spidermonkey_id;
            self.tools.toggle_breakpoint(&view, source, 5);
            self.tool_stage(15);
            return;
        }
        if stage == 15 && !self.tools.pending() {
            assert!(
                !self.tools.debug[&owner].breakpoints.is_empty(),
                "Engine resolves an executable breakpoint: {:?}",
                self.tools.notice
            );
            view.evaluate_javascript("fixtureRequests();'queued'", |_| {});
            self.tool_stage(16);
            return;
        }
        if stage == 16 && self.tools.debug[&owner].paused {
            assert_eq!(self.tools.debug[&owner].paused_line, Some(5));
            assert_eq!(
                self.tools.debug[&owner].paused_source,
                Some(self.tools.debug[&owner].breakpoints[0].0),
                "Paused source is selected for line highlighting"
            );
            self.tool_stage(17);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-debugger.png".into());
            self.window.request_redraw();
            return;
        }
        if stage == 17 && elapsed > Duration::from_millis(500) {
            assert!(
                (self.tools.panel_rect.height()
                    - self.harness.toolbox_test.as_ref().unwrap().height)
                    .abs()
                    < 3.0,
                "Toolbox retains height when switching panels"
            );
            let point = self.tools.debug[&owner].breakpoints[0];
            self.tools.toggle_breakpoint(&view, point.0, point.1);
            self.tools.resume(&owner, None);
            self.tools.run(
                &view,
                Tool::Inspector,
                crate::probes::edit_html(
                    "#heading",
                    "<h1 id=\"heading\">Edited through Inspector</h1>",
                ),
            );
            self.tool_stage(18);
            return;
        }
        if stage == 18 && !self.tools.pending() {
            assert!(
                self.tools.pages[&owner]
                    .html
                    .contains("Edited through Inspector"),
                "DOM edit reaches the real document"
            );
            self.tools.run(
                &view,
                Tool::StyleEditor,
                crate::probes::edit_css(0, "#heading { color: rgb(12,34,56); }"),
            );
            self.tool_stage(19);
            return;
        }
        if stage == 19 && !self.tools.pending() {
            self.tools.active = Tool::Inspector;
            self.tools.refresh(&view, &self.servo);
            self.tool_stage(20);
            return;
        }
        if stage == 20 && !self.tools.pending() {
            assert_eq!(
                self.tools.pages[&owner].snapshots[&Tool::Inspector].data["selected"]["styles"]["color"],
                "rgb(12, 34, 56)",
                "CSS override applied by the real style engine"
            );
            self.tools
                .run(&view, Tool::StyleEditor, crate::probes::RESET_CSS.into());
            self.tool_stage(21);
            return;
        }
        if stage == 21 && !self.tools.pending() {
            self.tools.refresh(&view, &self.servo);
            self.tool_stage(22);
            return;
        }
        if stage == 22 && !self.tools.pending() {
            assert_eq!(
                self.tools.pages[&owner].snapshots[&Tool::Inspector].data["selected"]["styles"]["color"],
                "rgb(103, 80, 151)",
                "CSS reset restores original sheet"
            );
            self.tools.run(
                &view,
                Tool::Storage,
                format!(
                    "localStorage.setItem('edited','verified');{}",
                    crate::probes::STORAGE
                ),
            );
            self.tool_stage(23);
            return;
        }
        if stage == 23 && !self.tools.pending() {
            assert!(
                self.tools.pages[&owner].snapshots[&Tool::Storage].data["local"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["key"] == "edited" && v["value"] == "verified"),
                "Real storage edit"
            );
            let composer = network::Composer {
                url: view
                    .url()
                    .unwrap()
                    .join("/api/composer")
                    .unwrap()
                    .to_string(),
                method: "POST".into(),
                headers: "Content-Type: text/plain".into(),
                body: "composer verified".into(),
            };
            view.evaluate_javascript(composer.script().unwrap(), |_| {});
            self.tool_stage(24);
            return;
        }
        if stage == 24 {
            let Some(row) = self
                .tools
                .network
                .rows
                .iter()
                .find(|r| r.url.ends_with("/api/composer") && r.status == Some(201))
            else {
                return;
            };
            assert_eq!(row.request_body.text(), Some("composer verified"));
            assert!(
                row.response_body
                    .text()
                    .unwrap()
                    .contains("composer verified")
            );
            let curl = row.curl().unwrap();
            assert!(curl.contains("--data-binary 'composer verified'"));
            let data = serde_json::to_vec(&self.tools.network.har(&owner)).unwrap();
            let exported = self
                .tools
                .network
                .rows
                .iter()
                .filter(|r| r.owner == owner)
                .count();
            let count = self.tools.network.import_har(&owner, &data).unwrap();
            assert_eq!(count, exported);
            assert!(count > 0);
            self.tools.network.query = "is:imported method:POST".into();
            self.tools.network.sort = network::Sort::Duration;
            self.tools.network.descending = true;
            let rows = self.tools.network.visible(&owner);
            assert!(!rows.is_empty());
            assert!(rows.iter().all(|r| r.imported && r.method == "POST"));
            self.tools.network.selected = Some(rows[0].uid);
            self.tools.network_ui.body_search = "verified".into();
            self.tools.detail_tab = 2;
            self.tools.active = Tool::Network;
            self.tool_stage(25);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-network-imported.png".into());
            self.window.request_redraw();
            return;
        }
        if stage == 25 && elapsed > Duration::from_millis(700) {
            self.tools.active = Tool::Inspector;
            self.tools.page(&view).selector = "#heading".into();
            self.tools.page(&view).inspector.tab = 1;
            self.tools.run(
                &view,
                Tool::Inspector,
                crate::inspector_probes::attribute(
                    "#heading",
                    "style",
                    Some("color:rgb(12,34,56);padding:17px;margin:9px;display:flex"),
                ),
            );
            self.tool_stage(26);
            return;
        }
        if stage == 26 && !self.tools.pending() {
            let snapshot = &self.tools.pages[&owner].snapshots[&Tool::Inspector];
            assert!(
                snapshot.error.is_none(),
                "Inspector error: {:?}",
                snapshot.error
            );
            let selected = &snapshot.data["selected"];
            assert_eq!(selected["styles"]["color"], "rgb(12, 34, 56)");
            assert_eq!(selected["box"]["padding"]["top"], "17px");
            assert_eq!(selected["layout"]["display"], "flex");
            assert!(!selected["rules"].as_array().unwrap().is_empty());
            assert!(selected["computed"].as_array().unwrap().len() >= 10);
            self.tool_stage(27);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("inspector-layout.png".into());
            self.window.request_redraw();
            return;
        }
        if stage == 27 && elapsed > Duration::from_millis(500) {
            self.tools.run(
                &view,
                Tool::Inspector,
                crate::inspector_probes::attribute("#heading", "id", Some("inspected-heading")),
            );
            self.tool_stage(28);
            return;
        }
        if stage == 28 && !self.tools.pending() {
            assert_eq!(self.tools.pages[&owner].selector, "#inspected-heading");
            self.tools.run(
                &view,
                Tool::Inspector,
                crate::inspector_probes::undo("#inspected-heading"),
            );
            self.tool_stage(29);
            return;
        }
        if stage == 29 && !self.tools.pending() {
            assert_eq!(self.tools.pages[&owner].selector, "#heading");
            self.tools.run(
                &view,
                Tool::Inspector,
                crate::inspector_probes::undo("#heading"),
            );
            self.tool_stage(30);
            return;
        }
        if stage == 30 && !self.tools.pending() {
            let snapshot = &self.tools.pages[&owner].snapshots[&Tool::Inspector];
            assert!(snapshot.error.is_none(), "Undo error: {:?}", snapshot.error);
            assert_ne!(
                snapshot.data["selected"]["styles"]["color"],
                "rgb(12, 34, 56)"
            );
            assert!(snapshot.data["changes"].as_array().unwrap().is_empty());
            self.tools
                .run(&view, Tool::Inspector, crate::inspector_probes::picker());
            self.chrome.page_focused = true;
            self.tool_stage(31);
            return;
        }
        if stage == 31 && elapsed > Duration::from_millis(500) {
            let rect =
                &self.tools.pages[&owner].snapshots[&Tool::Inspector].data["selected"]["box"];
            let x = rect["x"].as_f64().unwrap() + 20.0;
            let y = rect["y"].as_f64().unwrap() + rect["height"].as_f64().unwrap() / 2.0;
            self.smoke_click(x, y);
            self.tool_stage(32);
            return;
        }
        if stage == 32 && !self.tools.pending() {
            let snapshot = &self.tools.pages[&owner].snapshots[&Tool::Inspector];
            assert!(
                snapshot.error.is_none(),
                "Picker error: {:?}",
                snapshot.error
            );
            assert_eq!(self.tools.pages[&owner].selector, "#heading");
            self.tools.run(&view,Tool::Inspector,format!("const selector='#heading';if(globalThis.__wegaPickerCancel||document.querySelector(selector).style.outline)throw Error('Picker cleanup failed');{}",crate::probes::INSPECTOR));
            self.tool_stage(33);
            return;
        }
        if stage == 33 && !self.tools.pending() {
            assert!(
                self.tools.pages[&owner].snapshots[&Tool::Inspector]
                    .error
                    .is_none()
            );
            self.apply(wega::application::Command::Dock(
                crate::tool_window::Dock::Right,
            ));
            self.tools.page(&view).inspector.tab = 0;
            self.tool_stage(34);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-dock-right.png".into());
            self.window.request_redraw();
            return;
        }
        if stage == 34 && elapsed > Duration::from_millis(700) {
            self.assert_toolbox_side_dock(false);
            assert!(self.chrome.page_rect.width() >= 180.0);
            self.apply(wega::application::Command::Dock(
                crate::tool_window::Dock::Left,
            ));
            self.tool_stage(35);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-dock-left.png".into());
            self.window.request_redraw();
            return;
        }
        if stage == 35 && elapsed > Duration::from_millis(700) {
            self.assert_toolbox_side_dock(true);
            self.apply(wega::application::Command::Dock(
                crate::tool_window::Dock::Bottom,
            ));
            self.tools.split_console = true;
            self.tool_stage(36);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-split-console.png".into());
            self.window.request_redraw();
            return;
        }
        if stage == 36 && elapsed > Duration::from_millis(700) {
            assert!(
                self.tools.content_rect.max.y <= self.tools.split_rect.min.y + 1.0,
                "Split console has its own clipped region"
            );
            assert!(
                self.tools.panel_rect.min.y > self.chrome.page_rect.max.y,
                "Bottom dock leaves page above"
            );
            self.tools.split_console = false;
            self.apply(wega::application::Command::Dock(
                crate::tool_window::Dock::Window,
            ));
            self.tool_stage(37);
            self.window.request_redraw();
            return;
        }
        if stage == 37
            && elapsed > Duration::from_millis(700)
            && self.tool_window.as_ref().is_some_and(|w| w.frames > 1)
        {
            self.tool_stage(38);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("detached-inspector.png".into());
            self.tool_window.as_ref().unwrap().window.request_redraw();
            return;
        }
        if stage == 38 && elapsed > Duration::from_millis(700) {
            assert!(self.profile_dir.join("detached-inspector.png").exists());
            let full = self.harness.toolbox_test.as_ref().unwrap().page_height;
            assert!(
                full > 0.0 && (self.chrome.page_rect.height() - full).abs() < 2.0,
                "Detached toolbox restores full page height ({} of {full})",
                self.chrome.page_rect.height()
            );
            self.tools.active = Tool::Network;
            self.tool_stage(39);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("detached-network.png".into());
            self.tool_window.as_ref().unwrap().window.request_redraw();
            return;
        }
        if stage == 39 && elapsed > Duration::from_millis(700) {
            assert!(self.profile_dir.join("detached-network.png").exists());
            self.tool_window_event(winit::event::WindowEvent::CloseRequested);
            assert!(
                !self.chrome.show_console
                    && self.tool_window.is_none()
                    && !self.quit_requested
                    && !self.session().tabs().is_empty()
            );
            self.chrome.show_console = true;
            self.tool_stage(40);
            self.window.request_redraw();
            return;
        }
        if stage == 40
            && elapsed > Duration::from_millis(700)
            && self.tool_window.as_ref().is_some_and(|w| w.frames > 0)
        {
            self.apply(wega::application::Command::Dock(
                crate::tool_window::Dock::Left,
            ));
            self.tools.active = Tool::Inspector;
            self.tool_stage(41);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("toolbox-redocked.png".into());
            self.window.request_redraw();
            return;
        }
        if stage == 41 && elapsed > Duration::from_millis(700) && self.tool_window.is_none() {
            assert!(self.tool_window.is_none());
            self.assert_toolbox_side_dock(true);
            self.tools.refresh(&view, &self.servo);
            self.tool_stage(42);
            return;
        }
        if stage == 42 && !self.tools.pending() {
            let snapshot = &self.tools.pages[&owner].snapshots[&Tool::Inspector];
            assert!(snapshot.error.is_none());
            let width = snapshot.data["viewport"]["width"].as_f64().unwrap();
            assert!(
                (width - self.chrome.page_rect.width() as f64).abs() < 2.0,
                "Engine viewport {width} matches docked page {}",
                self.chrome.page_rect.width()
            );
            self.apply(wega::application::Command::Dock(
                crate::tool_window::Dock::Window,
            ));
            self.tool_stage(43);
            self.window.request_redraw();
            return;
        }
        if stage == 43 && self.tool_window.as_ref().is_some_and(|w| w.frames > 0) {
            self.tools.page(&view).inspector.picking = true;
            self.tools
                .run(&view, Tool::Inspector, crate::inspector_probes::picker());
            self.tool_stage(44);
            return;
        }
        if stage == 44 && elapsed > Duration::from_millis(500) {
            assert!(self.cancel_element_picker());
            self.tool_stage(45);
            return;
        }
        if stage == 45 && !self.tools.pending() {
            assert!(
                self.tools.pages[&owner].snapshots[&Tool::Inspector]
                    .error
                    .as_deref()
                    .is_some_and(|e| e.contains("cancelled"))
            );
            self.tools.run(&view,Tool::Inspector,format!("const selector='#heading';if(globalThis.__wegaPickerCancel)throw Error('Picker listener not removed');{}",crate::probes::INSPECTOR));
            self.tool_stage(46);
            return;
        }
        if stage == 46 && !self.tools.pending() {
            assert!(
                self.tools.pages[&owner].snapshots[&Tool::Inspector]
                    .error
                    .is_none()
            );
            self.tools.log.clear(&owner);
            self.tools.log.preserve = true;
            self.tools.active = Tool::Console;
            self.tools.page(&view).console_state.timestamps = true;
            view.evaluate_javascript("advancedLogs();setTimeout(function advancedUncaught(){throw Error('advanced-uncaught')},20);const style=document.createElement('style');style.textContent='body { non-existent-servo-test: xyz; }';document.head.appendChild(style);", |_|{});
            self.tool_stage(47);
            return;
        }
        if stage == 47 && elapsed > Duration::from_millis(1000) {
            let entries: Vec<_> = self
                .tools
                .log
                .entries
                .iter()
                .filter(|e| e.owner == owner)
                .collect();
            for (level, text) in [
                ("Info", "advanced-info"),
                ("Warn", "advanced-warning"),
                ("Error", "advanced-error"),
                ("Error", "advanced-uncaught"),
            ] {
                assert!(
                    entries
                        .iter()
                        .any(|e| e.level == level && e.text.contains(text)),
                    "Missing native console {level} {text}: {:?}",
                    entries
                        .iter()
                        .map(|e| (&e.level, &e.text))
                        .collect::<Vec<_>>()
                );
            }
            let info = entries
                .iter()
                .find(|e| e.text.starts_with("advanced-info"))
                .unwrap();
            assert!(info.source.ends_with("advanced-tools.js") && info.line > 0);
            assert!(
                info.details["arguments"][1]["ObjectValue"]["preview"]["ownProperties"].is_array(),
                "Native object preview: {}",
                info.details
            );
            assert!(entries.iter().any(|e| {
                e.text.starts_with("advanced-trace")
                    && e.details["stacktrace"]
                        .as_array()
                        .is_some_and(|s| !s.is_empty())
            }));
            assert!(entries.iter().any(|e| e.text.contains("advanced-numbers")
                && e.text.contains("NaN")
                && e.text.contains("Infinity")));
            assert!(
                entries.iter().any(|e| e.category == "CSS"),
                "CSS parse messages are subscribed"
            );
            let filter = crate::console_model::State {
                query: "advanced -warning".into(),
                ..Default::default()
            };
            assert!(filter.matches(info));
            assert!(
                !filter.matches(
                    entries
                        .iter()
                        .find(|e| e.text == "advanced-warning")
                        .unwrap()
                )
            );
            let info_id = info.id;
            self.tools
                .page(&view)
                .console_state
                .expanded
                .insert(info_id);
            self.tool_stage(48);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("detached-advanced-console.png".into());
            self.tool_window.as_ref().unwrap().window.request_redraw();
            return;
        }
        if stage == 48 && elapsed > Duration::from_millis(700) {
            let (source, line) = self.advanced_location(&owner);
            self.tools.toggle_breakpoint(&view, source, line);
            self.tool_stage(49);
            return;
        }
        if stage == 49 && !self.tools.pending() {
            assert_eq!(self.tools.debug[&owner].breakpoints.len(), 1);
            let debug = self.tools.debug.get_mut(&owner).unwrap();
            debug.advanced.watch_next = 2;
            debug.advanced.watches = vec![
                crate::debug_model::Watch {
                    id: 1,
                    expression: "outerSecret + localSecret".into(),
                    result: None,
                },
                crate::debug_model::Watch {
                    id: 2,
                    expression: "localSecret".into(),
                    result: None,
                },
            ];
            self.tools.active = Tool::Debugger;
            view.evaluate_javascript("advancedToolsProbe();'queued'", |_| {});
            self.tool_stage(50);
            return;
        }
        if stage == 50 && self.tools.debug[&owner].paused && !self.tools.pending() {
            let debug = &self.tools.debug[&owner];
            assert!(
                debug.advanced.stack.len() >= 2,
                "Native nested call stack: {:?}",
                debug.advanced.stack
            );
            assert!(debug.advanced.environment.is_some());
            assert_eq!(
                debug.advanced.watches[0].result.as_ref().unwrap()["value"]["NumberValue"],
                42.0,
                "Watch evaluates in paused lexical scope: {:?}",
                debug.advanced.watches[0].result
            );
            assert_eq!(
                debug.advanced.watches[1].result.as_ref().unwrap()["value"]["NumberValue"],
                2.0
            );
            assert!(
                self.tools
                    .scope_identifiers(&owner, "local")
                    .contains(&"localSecret".into()),
                "Native local-scope identifier completion"
            );
            self.tools.page(&view).console_input = "localSecret".into();
            self.tools.console(&view);
            self.tool_stage(51);
            return;
        }
        if stage == 51 && !self.tools.pending() {
            assert!(
                self.tools
                    .log
                    .entries
                    .iter()
                    .any(|e| e.owner == owner && e.category == "Result" && e.text == "2")
            );
            let parent = self.tools.debug[&owner].advanced.stack[1].clone();
            self.tools.select_debug_frame(&owner, parent);
            self.tools.page(&view).console_input = "outerSecret".into();
            self.tools.console(&view);
            self.tool_stage(61);
            return;
        }
        if stage == 61 && !self.tools.pending() {
            assert!(
                self.tools
                    .log
                    .entries
                    .iter()
                    .rev()
                    .find(|e| e.owner == owner && e.category == "Result")
                    .is_some_and(|e| e.text == "40"),
                "Console evaluates in selected parent frame"
            );
            assert_eq!(
                self.tools.debug[&owner].advanced.watches[0]
                    .result
                    .as_ref()
                    .unwrap()["hasException"],
                true,
                "Child locals are unavailable from the parent frame"
            );
            let top = self.tools.debug[&owner].advanced.stack[0].clone();
            self.tools.select_debug_frame(&owner, top);
            self.tool_stage(62);
            return;
        }
        if stage == 62 && !self.tools.pending() {
            assert_eq!(
                self.tools.debug[&owner].advanced.watches[0]
                    .result
                    .as_ref()
                    .unwrap()["value"]["NumberValue"],
                42.0
            );
            assert!(self.debugger_shortcut(
                &winit::keyboard::Key::Named(winit::keyboard::NamedKey::F10),
                false
            ));
            self.tool_stage(52);
            return;
        }
        if stage == 52 && self.tools.debug[&owner].paused && !self.tools.pending() {
            let (source, line) = self.advanced_location(&owner);
            assert!(self.tools.debug[&owner].paused_line.unwrap() >= line);
            self.tools
                .set_breakpoint_enabled(&owner, source, line, false);
            assert!(
                self.tools.debug[&owner]
                    .advanced
                    .disabled
                    .contains(&(source, line))
            );
            self.tools
                .set_breakpoint_enabled(&owner, source, line, true);
            self.tools.set_breakpoints_enabled(&owner, false);
            assert!(self.tools.debug[&owner].advanced.all_disabled);
            let (hits, truncated) =
                crate::debug_model::search(&self.tools.debug[&owner].sources, "localSecret", None);
            assert!(!truncated && hits.len() >= 3);
            self.tools.debug.get_mut(&owner).unwrap().advanced.hits = hits;
            self.tool_stage(53);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("detached-advanced-debugger.png".into());
            self.tool_window.as_ref().unwrap().window.request_redraw();
            return;
        }
        if stage == 53 && elapsed > Duration::from_millis(700) {
            let (source, _) = self.advanced_location(&owner);
            self.tools.ignore_source(&owner, source, true);
            assert!(self.tools.debug[&owner].advanced.ignored.contains(&source));
            assert!(self.debugger_shortcut(
                &winit::keyboard::Key::Named(winit::keyboard::NamedKey::F8),
                false
            ));
            self.tool_stage(54);
            return;
        }
        if stage == 54
            && elapsed > Duration::from_millis(500)
            && !self.tools.pending()
            && !self.tools.debug[&owner].paused
        {
            self.tools.page(&view).console_input = "window.advancedCalls".into();
            self.tools.console(&view);
            self.tool_stage(55);
            return;
        }
        if stage == 55 && !self.tools.pending() {
            assert!(
                self.tools
                    .log
                    .entries
                    .iter()
                    .rev()
                    .find(|e| e.owner == owner && e.category == "Result")
                    .is_some_and(|e| e.text == "42")
            );
            self.tools.set_breakpoints_enabled(&owner, true);
            self.tool_stage(56);
            return;
        }
        if stage == 56 && elapsed > Duration::from_millis(500) {
            self.tools.page(&view).console_input = "advancedToolsProbe()".into();
            self.tools.console(&view);
            self.tool_stage(57);
            return;
        }
        if stage == 57 && elapsed > Duration::from_millis(700) {
            assert!(
                !self.tools.debug[&owner].paused,
                "Ignored source must suppress its executable breakpoint"
            );
            if self.tools.pending() {
                return;
            }
            self.tools.page(&view).console_input = "await Promise.resolve(77)".into();
            self.tools.console(&view);
            self.tool_stage(58);
            return;
        }
        if stage == 58 && !self.tools.pending() {
            assert!(
                self.tools
                    .log
                    .entries
                    .iter()
                    .rev()
                    .find(|e| e.owner == owner && e.category == "Result")
                    .is_some_and(|e| e.text == "77"),
                "Top-level await expression"
            );
            self.tools.page(&view).console_input =
                "({get secret(){throw Error('getter executed')},value:42})".into();
            self.tools.console(&view);
            self.tool_stage(59);
            return;
        }
        if stage == 59 && !self.tools.pending() {
            assert!(
                self.tools
                    .log
                    .entries
                    .iter()
                    .rev()
                    .find(|e| e.owner == owner && e.category == "Result")
                    .is_some_and(|e| e.level != "Error"
                        && e.text.contains("Accessor")
                        && e.text.contains("42")),
                "Page evaluation preview does not invoke getters"
            );
            self.tools.clear_breakpoints(&owner);
            assert!(self.tools.debug[&owner].breakpoints.is_empty());
            self.tools.page(&view).console_state.native_evaluation = true;
            self.tools.page(&view).console_input = "window.advancedCalls".into();
            self.tools.console(&view);
            self.tool_stage(63);
            return;
        }
        if stage == 63 && !self.tools.pending() {
            assert!(
                self.tools
                    .log
                    .entries
                    .iter()
                    .rev()
                    .find(|e| e.owner == owner && e.category == "Result")
                    .is_some_and(|e| e.text == "84"),
                "Native global evaluator"
            );
            let (source, line) = self.advanced_location(&owner);
            self.tools.ignore_source(&owner, source, false);
            let url = self.tools.debug[&owner]
                .sources
                .iter()
                .find(|s| s.spidermonkey_id == source)
                .unwrap()
                .url
                .to_string();
            self.jump_to_source(&owner, &url, line);
            assert_eq!(self.tools.active, Tool::Debugger);
            self.tool_stage(60);
            return;
        }
        if stage == 60 && elapsed > Duration::from_millis(500) {
            self.tools.active = Tool::Performance;
            self.tools.run(
                &view,
                Tool::Performance,
                format!(
                    "{}{}",
                    crate::performance_ui::START,
                    crate::probes::PERFORMANCE
                ),
            );
            self.tool_stage(64);
            return;
        }
        if stage == 64 && !self.tools.pending() {
            assert_eq!(
                self.tools.pages[&owner].snapshots[&Tool::Performance].data["recording"]["running"],
                true
            );
            self.tools.run(&view, Tool::Performance, format!("performance.mark('panel-start');await new Promise(r=>setTimeout(r,100));performance.mark('panel-end');performance.measure('panel-measure','panel-start','panel-end');{}{}", crate::performance_ui::STOP, crate::probes::PERFORMANCE));
            self.tool_stage(65);
            return;
        }
        if stage == 65 && !self.tools.pending() {
            let snapshot = &self.tools.pages[&owner].snapshots[&Tool::Performance];
            assert!(
                snapshot.error.is_none(),
                "Performance recording: {:?}",
                snapshot.error
            );
            let data = &snapshot.data;
            assert_eq!(data["recording"]["running"], false);
            assert!(
                data["recording"]["frames"]
                    .as_array()
                    .is_some_and(|f| !f.is_empty())
            );
            assert!(
                data["measures"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["name"] == "panel-measure" && e["duration"].as_f64().unwrap() > 0.0)
            );
            self.tools.page(&view).performance.selected = Some(0);
            self.tool_stage(66);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("detached-performance.png".into());
            self.tool_window.as_ref().unwrap().window.request_redraw();
            return;
        }
        if stage == 66 && elapsed > Duration::from_millis(700) {
            assert!(self.profile_dir.join("detached-performance.png").exists());
            let page = &self.tools.pages[&owner];
            let data = &page.snapshots[&Tool::Performance].data;
            assert!(
                (page.performance.range_end
                    - (data["recording"]["end"].as_f64().unwrap()
                        - data["recording"]["start"].as_f64().unwrap()))
                .abs()
                    < 1.0,
                "Timeline fits the complete recording"
            );
            self.tools.active = Tool::StyleEditor;
            self.tools.refresh(&view, &self.servo);
            self.tool_stage(67);
            return;
        }
        if stage == 67 && !self.tools.pending() && elapsed > Duration::from_millis(700) {
            assert!(
                self.tools.pages[&owner].css.contains("h1"),
                "Selected stylesheet source loads automatically"
            );
            let [list, editor] = self.tools.style_rects;
            assert!(
                list.is_positive() && editor.is_positive() && list.right() <= editor.left(),
                "Stylesheet list and editor are side by side: {list:?} {editor:?}"
            );
            self.tool_stage(68);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("detached-style-editor.png".into());
            self.tool_window.as_ref().unwrap().window.request_redraw();
            return;
        }
        if stage == 68 && elapsed > Duration::from_millis(700) {
            assert!(self.profile_dir.join("detached-style-editor.png").exists());
            self.tools.active = Tool::Network;
            self.tools.network.selected = self
                .tools
                .network
                .rows
                .iter()
                .find(|r| r.owner == owner && r.status == Some(200))
                .map(|r| r.uid);
            self.tool_stage(69);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("detached-network-details.png".into());
            self.tool_window.as_ref().unwrap().window.request_redraw();
            return;
        }
        if stage == 69 && elapsed > Duration::from_millis(700) {
            let [list, details] = self.tools.network_rects;
            assert!(
                list.is_positive() && details.is_positive() && list.right() <= details.left(),
                "Request details are beside the request list: {list:?} {details:?}"
            );
            assert!(
                self.profile_dir
                    .join("detached-network-details.png")
                    .exists()
            );
            self.tools.active = Tool::Storage;
            self.tools.refresh(&view, &self.servo);
            self.tools.page(&view).storage_area = 2;
            self.tool_stage(73);
            return;
        }
        if stage == 73 && !self.tools.pending() {
            assert!(self.tools.pages[&owner].cookies.loaded);
            self.tool_stage(74);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("detached-storage-cookies.png".into());
            self.tool_window.as_ref().unwrap().window.request_redraw();
            return;
        }
        if stage == 74 && elapsed > Duration::from_millis(700) {
            assert!(
                self.profile_dir
                    .join("detached-storage-cookies.png")
                    .exists()
            );
            self.add_tab("data:text/html,<title>Opaque storage test</title><h1>Opaque origin</h1>");
            self.tools.active = Tool::Storage;
            self.tool_stage(70);
            return;
        }
        if stage == 70 && elapsed > Duration::from_millis(1000) {
            self.tools.refresh(&view, &self.servo);
            self.tool_stage(71);
            return;
        }
        if stage == 71 && !self.tools.pending() {
            let snapshot = &self.tools.pages[&owner].snapshots[&Tool::Storage];
            assert!(
                snapshot.error.is_none(),
                "Opaque storage must not fail the entire probe: {:?}",
                snapshot.error
            );
            assert_eq!(snapshot.data["opaque"], true);
            assert!(snapshot.data["local"]["error"].is_string());
            assert!(snapshot.data["session"]["error"].is_string());
            let cookies = &self.tools.pages[&owner].cookies;
            assert!(cookies.rows.is_empty() && !cookies.pending());
            assert!(
                cookies.error.is_some(),
                "Opaque origins cannot inherit cookies"
            );
            self.tool_stage(72);
            self.harness.toolbox_test.as_mut().unwrap().capture =
                Some("detached-storage-opaque.png".into());
            self.tool_window.as_ref().unwrap().window.request_redraw();
            return;
        }
        if stage == 72 && elapsed > Duration::from_millis(700) {
            assert!(
                self.profile_dir
                    .join("detached-storage-opaque.png")
                    .exists()
            );

            self.check_tool_ui_contracts();
            println!(
                "TOOLBOX TEST PASSED: real HTTP requests, POST payload/response, headers, 404, redirect, waterfall layout, HAR binary round trip/import/filter/sort, composed POST, cURL, DOM tree, matched CSS, computed styles, box model, native element picking/cleanup, attribute editing and undo, left/right/bottom docking, split console, separate OS window close/reopen/redock, CSS source/edit/reset, script sources, debugger pause/resume and executable breakpoints, console evaluation, structured severities/source locations/object previews/stack traces/CSS/page errors, paused lexical scopes, watches, paused-frame evaluation, step shortcuts, breakpoint toggles, source search/ignore, await expressions and accessor-safe previews, performance recording/timeline/frame intervals, right-hand Network/Style Editor panes, opaque-origin storage handling, memory reports, storage edits, actual accessibility nodes, application manifest"
            );
            self.harness.toolbox_test = None;
            event_loop.exit();
        }
    }
}
