use crate::{
    browser::Browser,
    capabilities::{self, Decision},
};
impl Browser {
    pub fn update_documents(&self) {
        let mut docs = self.documents.write().unwrap();
        let palette = self.profile().theme.palette(
            self.window.theme() != Some(winit::window::Theme::Light),
            false,
        );
        let document = |title: &str, body: &str| capabilities::document(title, body, palette);
        let shortcuts = self
            .profile()
            .bookmarks
            .iter()
            .take(8)
            .map(|b| {
                format!(
                    "<li><a href='{}'>{}</a></li>",
                    capabilities::html(&b.url),
                    capabilities::html(&b.title)
                )
            })
            .collect::<String>();
        let policy = &self.profile().policy;
        let (action, field, fixed) = policy.search_form();
        let engine = match policy.search_engine {
            wega::model::SearchEngine::Custom => "the web".to_owned(),
            engine => engine.name().to_owned(),
        };
        let hidden = fixed
            .iter()
            .map(|(name, value)| {
                format!(
                    "<input type='hidden' name='{}' value='{}'>",
                    capabilities::html(name),
                    capabilities::html(value)
                )
            })
            .collect::<String>();
        docs.insert("newtab".into(), document("Welcome to Wega", &format!("<p>Search here or press <b>Cmd/Ctrl+L</b> to enter an address.</p><form action='{}' method='get'><label for='search'>Search the web</label><p class='search-controls'>{hidden}<input id='search' name='{}' type='search' placeholder='Search {}' required><button type='submit'>Search</button></p></form><h2>Saved pages</h2><ul>{shortcuts}</ul><p><a href='wega://history'>Browse recent pages</a></p>", capabilities::html(&action), capabilities::html(&field), capabilities::html(&engine))));
        for (name, title, items) in [
            ("history", "History", &self.profile().history),
            ("bookmarks", "Bookmarks", &self.profile().bookmarks),
        ] {
            let list = items
                .iter()
                .map(|b| {
                    format!(
                        "<li><a href='{}'>{}</a></li>",
                        capabilities::html(&b.url),
                        capabilities::html(&b.title)
                    )
                })
                .collect::<String>();
            docs.insert(name.into(), document(title, &format!("<ul>{list}</ul>")));
        }
        docs.insert("settings".into(),document("Browser settings","<p>Use the Browser settings window to manage appearance, browsing, privacy, downloads and advanced options. If it is closed, reopen it from <b>… → Browser settings</b>.</p><p>Internal documents have no access to native browser controls.</p>"));
    }
    pub fn screenshot_page(&mut self) {
        let folder = self.profile_dir.join("screenshots");
        if let Err(e) = std::fs::create_dir_all(&folder) {
            self.status = Some(e.to_string());
            return;
        }
        let path = folder.join(format!(
            "page-{}.png",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let events = self.delegate.clone();
        self.view()
            .take_screenshot(None, move |result| match result {
                Ok(image) => match image.save(&path) {
                    Ok(()) => events.notice(
                        crate::ux::Severity::Success,
                        format!("Saved screenshot: {}", path.display()),
                        Some(path),
                    ),
                    Err(error) => {
                        events.notice(crate::ux::Severity::Error, error.to_string(), None)
                    }
                },
                Err(error) => events.notice(
                    crate::ux::Severity::Error,
                    format!("Screenshot failed: {error:?}"),
                    None,
                ),
            });
        self.notify(
            crate::ux::Severity::Info,
            "Waiting for the page screenshot…",
            None,
        );
    }
    pub fn reload_without_cache(&mut self) {
        if let Some(url) = self.view().url() {
            let mut headers = http::HeaderMap::new();
            headers.insert(
                http::header::CACHE_CONTROL,
                http::HeaderValue::from_static("no-cache"),
            );
            headers.insert(
                http::header::PRAGMA,
                http::HeaderValue::from_static("no-cache"),
            );
            self.view()
                .load_request(servo::UrlRequest::new(url).headers(headers));
        }
    }
    pub fn refresh_site_data(&mut self) {
        self.features.sites = self
            .servo
            .site_data_manager()
            .site_data(servo::StorageType::all())
            .into_iter()
            .map(|s| (s.name(), format!("{:?}", s.storage_types())))
            .collect();
        self.features.sites.sort_by(|a, b| a.0.cmp(&b.0));
        self.features.cookies = self
            .view()
            .url()
            .map(|url| {
                self.servo
                    .site_data_manager()
                    .cookies_for_url(url, servo::CookieSource::HTTP)
                    .iter()
                    .map(|c| {
                        format!(
                            "{}={} · domain={} · path={} · HttpOnly={} · Secure={}",
                            c.name(),
                            c.value(),
                            c.domain().unwrap_or(""),
                            c.path().unwrap_or("/"),
                            c.http_only().unwrap_or(false),
                            c.secure().unwrap_or(false)
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
    pub fn feature_ui(&mut self, ctx: &egui::Context) {
        if !self.dialogs.prompts.is_empty() {
            let fresh = self.prepare_dialog(ctx, "engine-confirmation");
            let mut answer = if crate::ux::escape(ctx) {
                Some(false)
            } else {
                None
            };
            let prompt = self.dialogs.prompts.front().unwrap();
            egui::Modal::new(egui::Id::unique("engine-confirmation")).show(ctx, |ui| {
                ui.set_max_width(480.0);
                ui.heading(&prompt.title);
                ui.label(&prompt.detail);
                ui.horizontal(|ui| {
                    let cancel_button = ui.button("Cancel");
                    if fresh {
                        cancel_button.request_focus();
                    }
                    if cancel_button.clicked() {
                        answer = Some(false);
                    }
                    if ui.button("Continue").clicked() {
                        answer = Some(true);
                    }
                });
            });
            if let Some(allow) = answer {
                let prompt = self.dialogs.prompts.pop_front().unwrap();
                self.finish_dialog(ctx);
                match prompt.decision {
                    Decision::Servo(request) => {
                        if allow {
                            request.allow()
                        } else {
                            request.deny()
                        }
                    }
                    Decision::Protocol(request, registration) => {
                        if allow {
                            match registration.register_or_unregister {
                                servo::RegisterOrUnregister::Register => {
                                    self.delegate
                                        .handlers
                                        .borrow_mut()
                                        .insert(registration.scheme, registration.url);
                                }
                                servo::RegisterOrUnregister::Unregister => {
                                    self.delegate
                                        .handlers
                                        .borrow_mut()
                                        .remove(&registration.scheme);
                                }
                            }
                            request.allow();
                        } else {
                            request.deny();
                        }
                    }
                    Decision::External(url) if allow => {
                        #[cfg(target_os = "macos")]
                        if let Err(error) = std::process::Command::new("/usr/bin/open")
                            .arg(url.as_str())
                            .spawn()
                        {
                            self.status = Some(error.to_string());
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            let _ = url;
                            self.status = Some(
                                "External application launching is currently available on macOS."
                                    .into(),
                            );
                        }
                    }
                    _ => {}
                }
            }
            return;
        }
        if !self.features.bluetooth.is_empty() {
            let fresh = self.prepare_dialog(ctx, "bluetooth");
            let devices = self.features.bluetooth.front().unwrap().devices().clone();
            let mut selected = None;
            let mut cancel = crate::ux::escape(ctx);
            egui::Modal::new(egui::Id::unique("bluetooth-device")).show(ctx, |ui| {
                ui.heading("Choose a Bluetooth device");
                if devices.is_empty() {
                    ui.label("No matching devices found.");
                }
                for (i, device) in devices.iter().enumerate() {
                    if ui
                        .button(format!("{} ({})", device.name, device.address))
                        .clicked()
                    {
                        selected = Some(i);
                    }
                }
                let button = ui.button("Cancel");
                if fresh {
                    button.request_focus();
                }
                cancel |= button.clicked();
            });
            if let Some(i) = selected {
                let _ = self
                    .features
                    .bluetooth
                    .pop_front()
                    .unwrap()
                    .pick_device(&devices[i]);
            } else if cancel {
                let _ = self.features.bluetooth.pop_front().unwrap().cancel();
            }
            return;
        }
        if self.features.settings {
            let mut open = true;
            egui::Window::new("Browser settings").open(&mut open)
                .default_width(620.0).max_width((ctx.content_rect().width()-40.0).max(280.0))
                .default_pos(egui::pos2(24.0,80.0)).show(ctx,|ui|{
                ui.horizontal_wrapped(|ui|{
                    for (i,label) in ["Appearance","Browsing","Privacy","Downloads","Advanced"].iter().enumerate() {
                        ui.selectable_value(&mut self.features.settings_section,i,*label);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-210.0).max(180.0)).show(ui,|ui|{
                    match self.features.settings_section {
                        0 => {
                            ui.heading("Appearance");
                            for theme in crate::theme::Theme::ALL {
                                if ui.selectable_label(self.profile().theme==theme,theme.name()).clicked(){self.set_theme(ctx,theme);}
                            }
                            ui.weak("Auto follows your system appearance. Website colors are controlled by each website.");
                            ui.horizontal(|ui|{
                                if ui.button("Tabs on top").clicked(){self.apply(wega::application::Command::Layout(crate::profile::TabLayout::Top));}
                                if ui.button("Tabs in sidebar").clicked(){self.apply(wega::application::Command::Layout(crate::profile::TabLayout::Side));}
                            });
                        }
                        1 => {
                    ui.heading("Navigation and requests");
                    ui.label("Host rules match the exact host and its subdomains. Separate hosts with spaces or newlines.");
                    ui.label("Blocked hosts (also blocks subresources)");
                    let mut policy = self.profile().policy.clone();
                    let mut changed=ui.add(egui::TextEdit::multiline(&mut policy.blocked_hosts).desired_rows(2).desired_width(f32::INFINITY)).changed();
                    ui.label("Allowed hosts (empty allows all; restricts navigation and subresources)");
                    changed|=ui.add(egui::TextEdit::multiline(&mut policy.allowed_hosts).desired_rows(2).desired_width(f32::INFINITY)).changed();
                    changed|=ui.checkbox(&mut policy.offline,"Offline mode — block HTTP/HTTPS requests").changed();
                    changed|=ui.checkbox(&mut policy.allow_window_changes,"Allow sites to move and resize this window").changed();
                    ui.separator();
                    ui.heading("Search");
                    egui::ComboBox::from_label("Search engine").selected_text(policy.search_engine.name()).show_ui(ui,|ui|{
                        for engine in wega::model::SearchEngine::ALL {
                            changed|=ui.selectable_value(&mut policy.search_engine,engine,engine.name()).changed();
                        }
                    });
                    if policy.search_engine==wega::model::SearchEngine::Custom {
                        ui.label("Search URL, with %s where the search terms go");
                        changed|=ui.add(egui::TextEdit::singleline(&mut policy.search_template).hint_text("https://search.example/?q=%s").desired_width(f32::INFINITY)).changed();
                        if !wega::model::valid_search_template(&policy.search_template) {
                            ui.colored_label(crate::theme::palette(ui.ctx()).amber,"Enter an http(s) URL containing %s once. Until then, Google is used.");
                        }
                    }
                    ui.separator();
                    ui.heading("Memory");
                    ui.horizontal_wrapped(|ui|{
                        ui.label("Unload background tabs after");
                        changed|=ui.add(egui::DragValue::new(&mut policy.discard_after_minutes).range(0..=wega::lifecycle::MAX_DISCARD_MINUTES).suffix(" min")).changed();
                    });
                    ui.weak("Unloaded tabs keep their place, title and icon, and reload when selected. Tabs playing media, waiting for a dialog or used by developer tools stay loaded. 0 keeps every tab loaded.");
                    if changed { self.apply(wega::application::Command::Policy(policy)); }

                        }
                        2 => {
                    ui.separator();ui.heading("Browsing data");
                    if ui.button("Review cookies and stored sites").clicked(){self.refresh_site_data();self.features.site_info=true;}
                    if ui.button("Clear browsing data…").clicked(){self.features.clear_data=true;}
                    if ui.button("Clear session cookies").clicked(){self.servo.site_data_manager().clear_session_cookies(None);self.notify(crate::ux::Severity::Success,"Session cookies cleared.",None);}

                        }
                        3 => {
                            ui.heading("Downloads");
                            ui.label(format!("Save folder: {}",self.downloads.directory.display()));
                            if ui.button("Open download manager").clicked(){self.download_ui.open=true;}
                            ui.weak("Open completed files or show them in their folder from Downloads. In Docker, use the viewer's Downloads button to save to your computer.");
                        }
                        _ => {
                    ui.separator(); ui.heading("User script");
                    ui.label("Runs in every subsequently loaded page. Changes apply after reload.");
                    ui.add(egui::TextEdit::multiline(&mut self.features.script_source).code_editor().desired_rows(7).desired_width(f32::INFINITY));
                    if ui.button("Apply script").clicked(){let mut policy=self.profile().policy.clone();policy.user_script=self.features.script_source.clone();self.apply(wega::application::Command::Policy(policy));
                        if let Some(script)=self.features.script.take(){self.content_manager.remove_script(script);}
                        if !self.profile().policy.user_script.trim().is_empty(){let script=std::rc::Rc::new(servo::UserScript::new(self.profile().policy.user_script.clone(),None));self.content_manager.add_script(script.clone());self.features.script=Some(script);}
                        self.changed();self.notify(crate::ux::Severity::Success,"User script saved. Reload a page to apply.",None);
                    }

                            if ui.button("Open developer diagnostics").clicked(){self.features.diagnostics=true;}
                        }
                    }
                });
            });
            self.features.settings = open;
        }
        if self.features.diagnostics {
            let mut open = true;
            egui::Window::new("Developer rendering diagnostics")
                .open(&mut open)
                .default_width(520.0)
                .show(ctx, |ui| {
                    ui.separator();
                    ui.heading("Rendering diagnostics");
                    ui.horizontal(|ui| {
                        for (name, option) in [
                            ("Profiler", servo::WebRenderDebugOption::Profiler),
                            (
                                "Texture cache",
                                servo::WebRenderDebugOption::TextureCacheDebug,
                            ),
                            (
                                "Render targets",
                                servo::WebRenderDebugOption::RenderTargetDebug,
                            ),
                        ] {
                            if ui.button(name).clicked() {
                                self.view().toggle_webrender_debugging(option);
                            }
                        }
                    });
                    if ui.button("Capture WebRender state").clicked() {
                        self.view().capture_webrender();
                        self.notify(
                            crate::ux::Severity::Info,
                            "WebRender capture requested; see engine log for output location.",
                            None,
                        );
                    }
                    if ui.button("Clear this tab’s back/forward history").clicked() {
                        self.view().clear_session_history();
                    }
                });
            self.features.diagnostics = open;
        }
        if self.features.site_info {
            let mut open = true;
            egui::Window::new("Site data")
                .open(&mut open)
                .default_width(620.0)
                .default_pos(egui::pos2(
                    ((ctx.content_rect().width() - 620.0) / 2.0).max(12.0),
                    110.0,
                ))
                .frame(
                    egui::Frame::window(&ctx.style_of(if crate::theme::palette(ctx).dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    }))
                    .inner_margin(16),
                )
                .show(ctx, |ui| {
                    ui.label(self.tab().url());
                    if ui.button("Refresh").clicked() {
                        self.refresh_site_data();
                    }
                    egui::ScrollArea::vertical()
                        .max_height(430.0)
                        .show(ui, |ui| {
                            ui.heading("Cookies for current URL (including HttpOnly)");
                            for cookie in &self.features.cookies {
                                ui.add(egui::Label::new(cookie).wrap());
                            }
                            if self.features.cookies.is_empty() {
                                ui.weak("No cookies for this URL.");
                            }
                            ui.separator();
                            ui.heading("Stored sites");
                            let mut remove = None;
                            for (site, kinds) in &self.features.sites {
                                ui.horizontal(|ui| {
                                    if ui.button("Delete").clicked() {
                                        remove = Some(site.clone());
                                    }
                                    ui.label(site);
                                    ui.weak(kinds);
                                });
                            }
                            if let Some(site) = remove {
                                self.features.delete_site = Some(site);
                            }
                        });
                });
            self.features.site_info = open;
        }
        if let Some(site) = self.features.delete_site.clone() {
            let fresh = self.prepare_dialog(ctx, "delete-site-data");
            let cancel = crate::ux::escape(ctx);
            egui::Modal::new(egui::Id::unique("delete-site-data")).show(ctx,|ui|{
                ui.heading("Delete this site's data?");
                ui.label(&site);
                ui.label("Cookies and offline storage for this site will be removed. You may be signed out. This cannot be undone.");
                ui.horizontal(|ui|{
                    let button=ui.button("Cancel");if fresh {button.request_focus();}
                    if button.clicked() || cancel {self.features.delete_site=None;}
                    if ui.button(egui::RichText::new("Delete site data").color(crate::theme::palette(ctx).danger)).clicked(){
                        self.servo.site_data_manager().clear_site_data(&[&site],servo::StorageType::all());
                        self.features.delete_site=None;self.refresh_site_data();self.notify(crate::ux::Severity::Success,"Site data deleted.",None);
                    }
                });
            });
            return;
        }
        if self.features.clear_data {
            let fresh = self.prepare_dialog(ctx, "clear-data");
            let cancel = crate::ux::escape(ctx);
            egui::Modal::new(egui::Id::unique("clear-data")).show(ctx,|ui|{
                ui.heading("Clear browsing data?");ui.label("Deletes cookies, local/session storage, HTTP cache and browsing history. Open sites may sign you out. Bookmarks remain.");
                ui.horizontal(|ui|{let button=ui.button("Cancel");if fresh {button.request_focus();}
if button.clicked() || cancel {self.features.clear_data=false;}
                if ui.button("Clear data").clicked(){
                    let sites=self.servo.site_data_manager().site_data(servo::StorageType::all());let names:Vec<_>=sites.iter().map(|s|s.name()).collect();let names:Vec<_>=names.iter().map(String::as_str).collect();
                    self.servo.site_data_manager().clear_site_data(&names,servo::StorageType::all());self.servo.site_data_manager().clear_cookies(None);self.servo.network_manager().clear_cache();self.apply(wega::application::Command::ClearHistory);self.features.cookies.clear();self.features.sites.clear();self.features.clear_data=false;self.changed();self.notify(crate::ux::Severity::Success,"Browsing data cleared.",None);
                }});
            });
        }
        if self.features.find_open {
            let mut open = true;
            egui::Window::new("Find in page")
                .open(&mut open)
                .anchor(egui::Align2::RIGHT_TOP, [-24.0, 120.0])
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let response = ui.text_edit_singleline(&mut self.features.find);
                        let enter =
                            response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        for (label, backward) in [("Previous", true), ("Next", false)] {
                            if ui.button(label).clicked() || (enter && !backward) {
                                let text = serde_json::to_string(&self.features.find).unwrap();
                                let source = include_str!("page_actions.js")
                                    .replace("BACKWARDS", if backward { "true" } else { "false" })
                                    .replace("QUERY", &text);
                                let delegate = self.delegate.clone();
                                self.view().evaluate_javascript(source, move |result| {
                                    if let Ok(servo::JSValue::String(text)) = result {
                                        delegate.notice(crate::ux::Severity::Info, text, None);
                                    }
                                });
                            }
                        }
                    });
                });
            self.features.find_open = open;
        }
    }
}
