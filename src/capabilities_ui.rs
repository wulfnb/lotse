use crate::{
    browser::Browser,
    capabilities::{self, Decision},
};
impl Browser {
    pub fn update_documents(&self) {
        let mut docs = self.documents.write().unwrap();
        docs.insert("newtab".into(), capabilities::document("Welcome to Lotse", "<p>Lotse is your native Rust browser, powered by Servo.</p><p><a href='https://www.google.com/'>Search the web</a></p>"));
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
            docs.insert(
                name.into(),
                capabilities::document(title, &format!("<ul>{list}</ul>")),
            );
        }
        docs.insert("settings".into(),capabilities::document("Browser settings","<p>Open <b>… → Browser settings</b> to manage host rules, offline mode, scripts and site data.</p><p>Internal documents have no access to native browser controls.</p>"));
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
        self.tab().view.take_screenshot(None, move |result| {
            let message = match result {
                Ok(image) => match image.save(&path) {
                    Ok(()) => format!("Saved screenshot: {}", path.display()),
                    Err(e) => e.to_string(),
                },
                Err(e) => format!("Screenshot failed: {e:?}"),
            };
            events.report(message);
        });
        self.status = Some("Waiting for the page screenshot…".into());
    }
    pub fn reload_without_cache(&mut self) {
        if let Some(url) = self.tab().view.url() {
            let mut headers = http::HeaderMap::new();
            headers.insert(
                http::header::CACHE_CONTROL,
                http::HeaderValue::from_static("no-cache"),
            );
            headers.insert(
                http::header::PRAGMA,
                http::HeaderValue::from_static("no-cache"),
            );
            self.tab()
                .view
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
            .tab()
            .view
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
        if let Some(prompt) = self.features.prompts.front() {
            let mut answer = None;
            egui::Modal::new(egui::Id::unique("engine-confirmation")).show(ctx, |ui| {
                ui.set_max_width(480.0);
                ui.heading(&prompt.title);
                ui.label(&prompt.detail);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                    if ui.button("Continue").clicked() {
                        answer = Some(true);
                    }
                });
            });
            if let Some(allow) = answer {
                let prompt = self.features.prompts.pop_front().unwrap();
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
        if let Some(request) = self.features.bluetooth.front() {
            let devices = request.devices().clone();
            let mut selected = None;
            let mut cancel = false;
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
                cancel = ui.button("Cancel").clicked();
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
            egui::Window::new("Browser settings").open(&mut open).default_width(620.0)
                .default_pos(egui::pos2(((ctx.content_rect().width()-620.0)/2.0).max(12.0),100.0))
                .frame(egui::Frame::window(&ctx.style_of(if crate::theme::palette(ctx).dark {egui::Theme::Dark}else{egui::Theme::Light})).inner_margin(16)).show(ctx,|ui|{
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-200.0).max(250.0))
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible).show(ui,|ui|{
                    ui.heading("Navigation and requests");
                    ui.label("Host rules match the exact host and its subdomains. Separate hosts with spaces or newlines.");
                    ui.label("Blocked hosts (also blocks subresources)");
                    let mut policy = self.profile().policy.clone();
                    let mut changed=ui.add(egui::TextEdit::multiline(&mut policy.blocked_hosts).desired_rows(2).desired_width(f32::INFINITY)).changed();
                    ui.label("Allowed hosts (empty allows all; restricts navigation and subresources)");
                    changed|=ui.add(egui::TextEdit::multiline(&mut policy.allowed_hosts).desired_rows(2).desired_width(f32::INFINITY)).changed();
                    changed|=ui.checkbox(&mut policy.offline,"Offline mode — block HTTP/HTTPS requests").changed();
                    changed|=ui.checkbox(&mut policy.allow_window_changes,"Allow sites to move and resize this window").changed();
                    if changed { self.apply(servo_browser::application::Command::Policy(policy)); }
                    ui.separator(); ui.heading("User script");
                    ui.label("Runs in every subsequently loaded page. Changes apply after reload.");
                    ui.add(egui::TextEdit::multiline(&mut self.features.script_source).code_editor().desired_rows(7).desired_width(f32::INFINITY));
                    if ui.button("Apply script").clicked(){let mut policy=self.profile().policy.clone();policy.user_script=self.features.script_source.clone();self.apply(servo_browser::application::Command::Policy(policy));
                        if let Some(script)=self.features.script.take(){self.content_manager.remove_script(script);}
                        if !self.profile().policy.user_script.trim().is_empty(){let script=std::rc::Rc::new(servo::UserScript::new(self.profile().policy.user_script.clone(),None));self.content_manager.add_script(script.clone());self.features.script=Some(script);}
                        self.changed();self.status=Some("User script saved. Reload a page to apply.".into());
                    }
                    ui.separator();ui.heading("Browsing data");
                    if ui.button("Review cookies and stored sites").clicked(){self.refresh_site_data();self.features.site_info=true;}
                    if ui.button("Clear browsing data…").clicked(){self.features.clear_data=true;}
                    if ui.button("Clear session cookies").clicked(){self.servo.site_data_manager().clear_session_cookies(None);self.status=Some("Session cookies cleared.".into());}
                    ui.separator();ui.heading("Media session");
                    if let Some(event)=self.features.media.get(&self.tab().view.id()){ui.label(format!("{event:?}"));}
                    ui.horizontal(|ui|{for (name,action) in [("Play",servo::MediaSessionActionType::Play),("Pause",servo::MediaSessionActionType::Pause),("Previous",servo::MediaSessionActionType::PreviousTrack),("Next",servo::MediaSessionActionType::NextTrack)]{if ui.button(name).clicked(){self.tab().view.notify_media_session_action_event(action);}}});
                    ui.separator();ui.heading("Rendering diagnostics");
                    ui.horizontal(|ui|{for (name,option) in [("Profiler",servo::WebRenderDebugOption::Profiler),("Texture cache",servo::WebRenderDebugOption::TextureCacheDebug),("Render targets",servo::WebRenderDebugOption::RenderTargetDebug)]{if ui.button(name).clicked(){self.tab().view.toggle_webrender_debugging(option);}}});
                    if ui.button("Capture WebRender state").clicked(){self.tab().view.capture_webrender();self.status=Some("WebRender capture requested; see engine log for output location.".into());}
                    if ui.button("Clear this tab’s back/forward history").clicked(){self.tab().view.clear_session_history();}
                });
            });
            self.features.settings = open;
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
                                self.servo
                                    .site_data_manager()
                                    .clear_site_data(&[&site], servo::StorageType::all());
                                self.refresh_site_data();
                            }
                        });
                });
            self.features.site_info = open;
        }
        if self.features.clear_data {
            egui::Modal::new(egui::Id::unique("clear-data")).show(ctx,|ui|{
                ui.heading("Clear browsing data?");ui.label("Deletes cookies, local/session storage, HTTP cache and browsing history. Open sites may sign you out. Bookmarks remain.");
                ui.horizontal(|ui|{if ui.button("Cancel").clicked(){self.features.clear_data=false;}
                if ui.button("Clear data").clicked(){
                    let sites=self.servo.site_data_manager().site_data(servo::StorageType::all());let names:Vec<_>=sites.iter().map(|s|s.name()).collect();let names:Vec<_>=names.iter().map(String::as_str).collect();
                    self.servo.site_data_manager().clear_site_data(&names,servo::StorageType::all());self.servo.site_data_manager().clear_cookies(None);self.servo.network_manager().clear_cache();self.apply(servo_browser::application::Command::ClearHistory);self.features.cookies.clear();self.features.sites.clear();self.features.clear_data=false;self.changed();self.status=Some("Browsing data cleared.".into());
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
                                self.tab().view.evaluate_javascript(source, move |result| {
                                    if let Ok(servo::JSValue::String(text)) = result {
                                        delegate.report(text);
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
