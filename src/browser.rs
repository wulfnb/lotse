use crate::{
    Options,
    engine::{self, Event, Wake},
    input,
    profile::{self, Bookmark, Profile, TabLayout},
};
use servo::{InputEvent, RenderingContext, WebView, WebViewBuilder, WindowRenderingContext};
use servo_browser::{
    application::{Application, Command as AppCommand, Event as AppEvent},
    session::{Command, Session},
    storage::FileProfileStore,
};
use std::cell::RefCell;
use std::{
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Fullscreen, Window},
};

pub struct Tab {
    pub(crate) view: WebView,
    pub(crate) surface: Rc<servo::OffscreenRenderingContext>,
    pub(crate) initial_url: String,
    pub(crate) crash: Option<String>,
    pub(crate) history: servo_browser::navigation::History,
}

impl Tab {
    pub(crate) fn url(&self) -> String {
        self.view
            .url()
            .map(|u| u.to_string())
            .unwrap_or_else(|| self.initial_url.clone())
    }
    pub(crate) fn title(&self) -> String {
        self.view
            .page_title()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                self.view
                    .url()
                    .and_then(|u| u.host_str().map(str::to_owned))
                    .unwrap_or_else(|| "New tab".into())
            })
    }
}

pub struct Browser {
    pub(crate) chrome: crate::chrome::Chrome,
    pub(crate) automation: Option<crossbeam_channel::Receiver<crate::automation::Job>>,
    pub(crate) automation_navigation: Option<crate::automation::PendingNavigation>,
    #[cfg(feature = "test-harness")]
    pub(crate) capabilities_test: Option<crate::capabilities_test::Test>,
    pub(crate) favicons: std::collections::HashMap<String, egui::TextureHandle>,
    pub(crate) features: crate::capabilities::State,
    pub(crate) documents: crate::capabilities::Documents,
    pub(crate) window: Rc<Window>,
    pub(crate) native_media: crate::native_media::NativeMedia,
    pub(crate) native_theme: Option<crate::native_theme::NativeTheme>,
    session: Session<Tab>,
    pub(crate) servo: servo::Servo,
    pub(crate) content_manager: Rc<servo::UserContentManager>,
    pub(crate) context: Rc<WindowRenderingContext>,
    pub(crate) delegate: Rc<engine::Delegate>,
    pub(crate) gui: Option<egui_glow::EguiGlow>,
    application: Application,
    pub(crate) profile_dir: PathBuf,
    last_window_title: String,
    pub(crate) tool_window: Option<crate::tool_window::ToolWindow>,
    pub(crate) tools: crate::toolbox::Toolbox,
    #[cfg(feature = "test-harness")]
    pub(crate) toolbox_test: Option<crate::toolbox_test::Test>,
    #[cfg(feature = "test-harness")]
    pub(crate) theme_test: Option<crate::theme_test::Test>,
    pub(crate) console: VecDeque<(servo::WebViewId, String)>,
    permissions: servo_browser::permissions::Permissions<crate::permission_adapter::Permission>,
    pub(crate) authentications: VecDeque<servo::AuthenticationRequest>,
    pub(crate) controls: VecDeque<(servo::WebViewId, servo::EmbedderControl)>,
    pub(crate) status: Option<String>,
    pub(crate) resources: crate::resources::Monitor,
    pub(crate) downloads: crate::downloads::Manager,
    pub(crate) download_ui: crate::downloads_ui::State,
    #[cfg(feature = "test-harness")]
    pub(crate) download_test: Option<crate::download_test::Test>,
    pub(crate) fullscreen: bool,
    pub(crate) quit_requested: bool,
    pub(crate) repaint_deadline: Arc<std::sync::Mutex<Option<Instant>>>,
    pub(crate) pending_save: Option<Instant>,
    #[cfg(feature = "test-harness")]
    pub(crate) smoke: Option<Smoke>,
    pub(crate) capture: Option<crate::capture::Capture>,
}

#[cfg(feature = "test-harness")]
pub struct Smoke {
    pub(crate) start: Instant,
    pub(crate) stage: u8,
    pub(crate) stage_at: Instant,
    pub(crate) result:
        Rc<RefCell<Option<Result<servo::JSValue, servo::JavaScriptEvaluationError>>>>,
}

impl Browser {
    pub(crate) fn new(
        window: Window,
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<Wake>,
        options: &Options,
    ) -> Result<Self, String> {
        let window = Rc::new(window);
        let profile_dir = options
            .profile_dir
            .clone()
            .unwrap_or_else(Profile::default_dir);
        let mut downloads = crate::downloads::Manager::new(&profile_dir);
        if options.download_test {
            downloads.directory = profile_dir.join("test-downloads");
        }
        std::fs::create_dir_all(&profile_dir).map_err(|e| format!("Cannot create profile: {e}"))?;
        let (application, warning) =
            Application::load(Box::new(FileProfileStore::open(&profile_dir)?));
        let profile = application.profile();
        let context = Rc::new(
            WindowRenderingContext::new(
                event_loop.display_handle().map_err(|e| e.to_string())?,
                window.window_handle().map_err(|e| e.to_string())?,
                window.inner_size(),
            )
            .map_err(|e| format!("Cannot create GPU context: {e:?}"))?,
        );
        context
            .make_current()
            .map_err(|e| format!("Cannot activate GPU context: {e:?}"))?;
        let gui = egui_glow::EguiGlow::new(
            event_loop,
            context.glow_gl_api(),
            None,
            Some(window.scale_factor() as f32),
            false,
        );
        let native_theme = crate::native_theme::NativeTheme::new(&window).ok();
        let system_dark = window.theme() != Some(winit::window::Theme::Light);
        if let Some(native) = &native_theme {
            native.apply(profile.theme, profile.theme.dark(system_dark));
        }
        crate::theme::install(
            &gui.egui_ctx,
            profile.theme.palette(system_dark, native_theme.is_some()),
        );
        let repaint_proxy = proxy.clone();
        let repaint_deadline = Arc::new(std::sync::Mutex::new(None::<Instant>));
        let deadline = repaint_deadline.clone();
        gui.egui_ctx.set_request_repaint_callback(move |info| {
            if let Some(when) = Instant::now().checked_add(info.delay) {
                let mut deadline = deadline.lock().unwrap();
                if deadline.is_none_or(|current| when < current) {
                    *deadline = Some(when);
                }
            }
            if info.delay.is_zero() {
                let _ = repaint_proxy.send_event(Wake::Ui);
            }
        });
        let engine_dir = profile_dir.join("engine");
        std::fs::create_dir_all(&engine_dir).map_err(|e| e.to_string())?;
        let opts = servo::Opts {
            config_dir: Some(engine_dir),
            multiprocess: options.multiprocess,
            ..Default::default()
        };
        let tools = crate::toolbox::Toolbox::new(proxy.clone());
        let native_media = crate::native_media::NativeMedia::new(proxy.clone());
        let documents = Default::default();
        let mut protocols = servo::protocol_handler::ProtocolRegistry::default();
        for scheme in ["lotse", "servo"] {
            protocols
                .register(
                    scheme,
                    crate::capabilities::InternalPages(std::sync::Arc::clone(&documents)),
                )
                .map_err(|e| format!("Internal protocol: {e:?}"))?;
        }
        let automation = options
            .webdriver_port
            .map(|port| crate::automation::start(port, proxy.clone()))
            .transpose()?;
        let mut builder = servo::ServoBuilder::default()
            .protocol_registry(protocols)
            .preferences(servo::Preferences {
                devtools_server_enabled: options.remote_devtools.is_some(),
                accessibility_enabled: true,
                devtools_server_listen_address: format!(
                    "127.0.0.1:{}",
                    options.remote_devtools.unwrap_or(7000)
                ),
                ..Default::default()
            })
            .opts(opts)
            .event_loop_waker(Box::new(engine::Waker(proxy)));
        if options.remote_devtools.is_none() {
            builder = builder.devtools_event_sender(tools.bridge.sender.clone());
        }
        let servo = builder.build();
        let content_manager = crate::compat::content_manager(&servo);
        let delegate = Rc::new(engine::Delegate {
            active_view: std::cell::Cell::new(None),
            window: window.clone(),
            events: RefCell::new(VecDeque::new()),
            policy: RefCell::new(profile.policy.clone()),
            handlers: RefCell::new(Default::default()),
        });
        servo.set_delegate(delegate.clone());
        let mut features = crate::capabilities::State {
            script_source: profile.policy.user_script.clone(),
            ..Default::default()
        };
        if !profile.policy.user_script.trim().is_empty() {
            let script = Rc::new(servo::UserScript::new(
                profile.policy.user_script.clone(),
                None,
            ));
            content_manager.add_script(script.clone());
            features.script = Some(script);
        }
        let favicons = profile
            .favicons
            .iter()
            .filter_map(|(url, image)| {
                if image.width == 0
                    || image.height == 0
                    || image.width > 64
                    || image.height > 64
                    || image.rgba.len() != image.width * image.height * 4
                {
                    return None;
                }
                Some((
                    url.clone(),
                    gui.egui_ctx.load_texture(
                        "favicon",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [image.width, image.height],
                            &image.rgba,
                        ),
                        egui::TextureOptions::LINEAR,
                    ),
                ))
            })
            .collect();
        let mut browser = Self {
            chrome: Default::default(),
            #[cfg(feature = "test-harness")]
            capabilities_test: options
                .capabilities_test
                .then(crate::capabilities_test::Test::new),
            automation,
            automation_navigation: None,
            features,
            documents,
            favicons,
            window,
            native_theme,
            native_media,
            session: Session::default(),
            servo,
            content_manager,
            context,
            delegate,
            gui: Some(gui),
            application,
            profile_dir,
            last_window_title: String::new(),
            resources: crate::resources::Monitor::new(),
            downloads,
            download_ui: Default::default(),
            #[cfg(feature = "test-harness")]
            download_test: options.download_test.then(crate::download_test::Test::new),
            tool_window: None,
            tools,
            #[cfg(feature = "test-harness")]
            toolbox_test: options.toolbox_test.then(crate::toolbox_test::Test::new),
            #[cfg(feature = "test-harness")]
            theme_test: options.theme_test.then(crate::theme_test::Test::new),
            console: VecDeque::new(),
            permissions: Default::default(),
            authentications: VecDeque::new(),
            controls: VecDeque::new(),
            status: if options.remote_devtools.is_some() {
                Some("Remote DevTools mode: native Network/Debugger recording is disabled. Connect Firefox via about:debugging.".into())
            } else {
                warning
            },
            fullscreen: false,
            quit_requested: false,
            repaint_deadline,
            pending_save: None,
            #[cfg(feature = "test-harness")]
            smoke: options.smoke_test.then(|| Smoke {
                start: Instant::now(),
                stage_at: Instant::now(),
                stage: 0,
                result: Rc::new(RefCell::new(None)),
            }),
            capture: options
                .capture
                .as_ref()
                .map(|path| crate::capture::Capture::new(path.clone())),
        };
        let urls = if (options.smoke_test || options.theme_test || options.capabilities_test)
            && options.url.is_none()
        {
            vec![
                url::Url::from_file_path(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/interaction.html"),
                )
                .map_err(|_| "Invalid fixture path")?
                .to_string(),
            ]
        } else if options.toolbox_test && options.url.is_none() {
            vec!["http://127.0.0.1:8767/toolbox.html".into()]
        } else if let Some(url) = &options.url {
            vec![profile::resolve_address(url)?.to_string()]
        } else {
            browser.profile().urls.clone()
        };
        let saved_active = browser.profile().active;
        for address in urls.iter().take(100) {
            if let Ok(url) = profile::resolve_address(address) {
                browser.add_tab_url(url, false);
            }
        }
        if browser.session.tabs().is_empty() {
            browser.add_tab(profile::HOME);
        }
        browser.select_tab(saved_active.min(browser.session.tabs().len() - 1));
        browser.window.set_ime_allowed(true);
        browser.window.request_redraw();
        Ok(browser)
    }

    pub(crate) fn session(&self) -> &Session<Tab> {
        &self.session
    }
    pub(crate) fn move_tab(&mut self, from: usize, to: usize) {
        if !matches!(
            self.session.apply(Command::Move { from, to }),
            servo_browser::session::Event::Unchanged
        ) {
            self.changed();
        }
    }
    pub(crate) fn pending_permission(
        &self,
    ) -> Option<(servo_browser::permissions::RequestId, &str, &str)> {
        self.permissions.front()
    }
    pub(crate) fn answer_permission(
        &mut self,
        id: servo_browser::permissions::RequestId,
        allow: bool,
    ) {
        self.permissions.answer(id, allow);
    }
    pub(crate) fn profile(&self) -> &Profile {
        self.application.profile()
    }

    pub(crate) fn apply(&mut self, command: AppCommand) {
        match self.application.apply(command) {
            AppEvent::PolicyChanged => {
                *self.delegate.policy.borrow_mut() = self.profile().policy.clone();
                self.changed();
            }
            AppEvent::Changed => self.changed(),
            AppEvent::Unchanged => {}
        }
    }

    pub(crate) fn start_download(
        &mut self,
        url: &str,
        filename: &str,
        connections: usize,
        cookies: String,
    ) -> Result<(), String> {
        self.application
            .start_download(&mut self.downloads, url, filename, connections, cookies)
    }

    pub(crate) fn tab(&self) -> &Tab {
        &self.session.tabs()[self.session.active()]
    }

    pub(crate) fn add_tab(&mut self, address: &str) {
        match profile::resolve_address(address) {
            Ok(url) => self.add_tab_url(url, true),
            Err(error) => self.status = Some(error),
        }
    }

    fn add_tab_url(&mut self, url: url::Url, activate: bool) {
        let surface = Rc::new(self.context.offscreen_context(PhysicalSize::new(1000, 700)));
        let view = WebViewBuilder::new(&self.servo, surface.clone())
            .user_content_manager(self.content_manager.clone())
            .url(url.clone())
            .hidpi_scale_factor(euclid::Scale::new(self.window.scale_factor() as f32))
            .delegate(self.delegate.clone())
            .clipboard_delegate(Rc::new(engine::Clipboard))
            .build();
        view.notify_theme_change(servo::Theme::Dark);
        self.session.push(Tab {
            view,
            surface,
            initial_url: url.to_string(),
            crash: None,
            history: Default::default(),
        });
        if activate {
            self.select_tab(self.session.tabs().len() - 1);
        } else {
            self.session.tabs().last().unwrap().view.hide();
        }
        self.changed();
    }

    pub(crate) fn select_tab(&mut self, index: usize) {
        if index >= self.session.tabs().len() {
            return;
        }
        self.release_pointer();
        for (i, tab) in self.session.tabs().iter().enumerate() {
            if i == index {
                tab.view.show();
            } else {
                tab.view.hide();
            }
            tab.view.set_focused(false);
        }
        self.session.apply(Command::Select(index));
        self.delegate.active_view.set(Some(self.tab().view.id()));
        self.chrome.page_focused = false;
        self.chrome.address = self.tab().url();
        self.chrome.last_url = self.chrome.address.clone();
        self.sync_window_title();
        self.changed();
    }

    pub(crate) fn close_tab(&mut self, index: usize) {
        if index >= self.session.tabs().len() {
            return;
        }
        let was_active = index == self.session.active();
        if was_active {
            self.release_pointer();
        }
        self.chrome
            .closed_tabs
            .push(self.session.tabs()[index].url());
        if self.chrome.closed_tabs.len() > 30 {
            self.chrome.closed_tabs.remove(0);
        }
        self.features
            .media
            .remove(&self.session.tabs()[index].view.id());
        if self.features.media_owner == Some(self.session.tabs()[index].view.id()) {
            self.features.media_owner = None;
            self.native_media.clear();
        }
        self.tools.closed(self.session.tabs()[index].view.id());
        self.session.apply(Command::Close(index));
        if self.session.tabs().is_empty() {
            self.add_tab("about:blank");
            self.chrome.focus_address = true;
        } else {
            let index = self.session.active();
            self.select_tab(index);
        }
        self.changed();
    }

    pub(crate) fn media_action(&self, action: Option<servo::MediaSessionActionType>) {
        let Some(owner) = self.features.media_owner else {
            return;
        };
        if let Some(tab) = self.session.tabs().iter().find(|t| t.view.id() == owner) {
            let playing = self.features.media.get(&owner).is_some_and(|m| m.playing);
            tab.view
                .notify_media_session_action_event(action.unwrap_or(if playing {
                    servo::MediaSessionActionType::Pause
                } else {
                    servo::MediaSessionActionType::Play
                }));
        }
    }
    pub(crate) fn dispatch_navigation(
        &mut self,
        command: servo_browser::navigation::Command,
    ) -> Result<bool, String> {
        let view = self.tab().view.clone();
        let result = servo_browser::navigation::dispatch(
            command,
            &mut crate::navigation_adapter::Navigation {
                view: &view,
                traversals: &mut self.features.traversals,
            },
        );
        self.window.request_redraw();
        result
    }
    pub(crate) fn reload(&mut self) {
        let _ = self.dispatch_navigation(servo_browser::navigation::Command::Reload);
    }
    pub(crate) fn reset_zoom(&mut self) {
        let _ = self.dispatch_navigation(servo_browser::navigation::Command::ResetZoom);
    }
    pub(crate) fn traverse(&mut self, forward: bool) {
        let _ = self.dispatch_navigation(servo_browser::navigation::Command::Traverse { forward });
    }
    pub(crate) fn navigate(&mut self, address: &str) {
        match profile::resolve_address(address) {
            Ok(url) => {
                let active = self.session.active();
                self.session.tabs_mut()[active].crash = None;
                self.chrome.address = url.to_string();
                self.chrome.last_url = self.chrome.address.clone();
                self.dispatch_navigation(servo_browser::navigation::Command::Load(url.to_string()))
                    .expect("validated address");
                self.chrome.page_focused = true;
                self.tab().view.set_focused(true);
                self.status = None;
                self.changed();
            }
            Err(error) => self.status = Some(error),
        }
    }

    pub(crate) fn toggle_layout(&mut self) {
        self.apply(AppCommand::Layout(match self.profile().layout {
            TabLayout::Side => TabLayout::Top,
            TabLayout::Top => TabLayout::Side,
        }));
        self.changed();
    }

    pub(crate) fn set_theme(&mut self, ctx: &egui::Context, theme: crate::theme::Theme) {
        self.apply(AppCommand::Theme(theme));
        if theme == crate::theme::Theme::Glass
            && let Some(native) = &self.native_theme
        {
            native.apply(theme, false);
        }
        let dark = theme.dark(self.window.theme() != Some(winit::window::Theme::Light));
        if let Some(native) = &self.native_theme {
            native.apply(theme, dark);
        }
        crate::theme::install(ctx, theme.palette(dark, self.native_theme.is_some()));
        self.changed();
    }
    pub(crate) fn bookmark(&mut self) {
        let url = self.tab().url();
        self.apply(AppCommand::ToggleBookmark {
            title: self.tab().title(),
            url,
        });
        self.changed();
    }

    pub(crate) fn changed(&mut self) {
        self.update_documents();
        if self.pending_save.is_none() {
            self.pending_save = Some(Instant::now());
        }
        self.window.request_redraw();
    }

    pub(crate) fn save(&mut self) {
        let urls = self.session.tabs().iter().map(Tab::url).collect();
        if let Err(error) = self.application.save_session(urls, self.session.active()) {
            self.status = Some(format!("Could not save browser profile: {error}"));
            eprintln!("{}", self.status.as_deref().unwrap());
        }
        self.pending_save = None;
    }

    pub(crate) fn pump(&mut self) {
        let jobs: Vec<_> = self
            .automation
            .as_ref()
            .map(|rx| rx.try_iter().take(32).collect())
            .unwrap_or_default();
        for job in jobs {
            job(self);
        }
        for message in crate::native_notifications::updates() {
            self.status = Some(message);
        }
        self.servo.spin_event_loop();
        self.tools.pump();
        if !self.chrome.show_console && self.tools.network.disable_cache {
            self.servo
                .set_preference("network_http_cache_disabled", servo::PrefValue::Bool(false));
            self.tools.network.disable_cache = false;
        }
        if !self.chrome.show_console && self.tools.attention.is_none() {
            self.tools.resume_all();
            for tab in self.session.tabs() {
                tab.view.set_accessibility_active(false);
            }
        }
        if let Some(owner) = self.tools.attention.take() {
            if let Some(index) = self
                .session
                .tabs()
                .iter()
                .position(|tab| crate::network::owner(tab.view.id()) == owner)
            {
                self.select_tab(index);
            }
            self.chrome.show_console = true;
            self.tools.active = crate::toolbox::Tool::Debugger;
        }
        let events: Vec<_> = self.delegate.events.borrow_mut().drain(..).collect();
        for event in events {
            match event {
                Event::Download(url, name) => self.offer_download(&url, &name),
                Event::Load(owner, status) => self.automation_load_status(owner, status),
                Event::History(owner, urls, active) => {
                    if let Some(tab) = self
                        .session
                        .tabs_mut()
                        .iter_mut()
                        .find(|tab| tab.view.id() == owner)
                    {
                        tab.history.update(urls, active);
                    }
                }
                Event::Traversal(owner, id) => {
                    self.automation_traversal(owner, &id);
                    if self.features.traversals.get(&owner) == Some(&id) {
                        self.features.traversals.remove(&owner);
                    }
                    self.window.request_redraw();
                }
                Event::Bluetooth(request) => self.features.bluetooth.push_back(request),
                Event::Media(id, event) => {
                    let info = self.features.media.entry(id).or_default();
                    info.update(event);
                    if info.playing {
                        self.features.media_owner = Some(id);
                    }
                    if self.features.media_owner == Some(id) {
                        self.native_media.update(info);
                    }
                    self.window.request_redraw();
                }
                Event::Favicon(url, image) => {
                    let width = image.width as usize;
                    let height = image.height as usize;
                    let pixels = match image.format {
                        servo::PixelFormat::RGBA8 => image.data().to_vec(),
                        servo::PixelFormat::BGRA8 => image
                            .data()
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .flat_map(|p| [p[2], p[1], p[0], p[3]])
                            .collect(),
                        servo::PixelFormat::RGB8 => image
                            .data()
                            .as_chunks::<3>()
                            .0
                            .iter()
                            .flat_map(|p| [p[0], p[1], p[2], 255])
                            .collect(),
                        _ => Vec::new(),
                    };
                    if width > 0
                        && height > 0
                        && pixels.len() == width * height * 4
                        && let Some(gui) = &self.gui
                    {
                        if self.favicons.len() >= 1000 {
                            self.favicons.clear();
                        }
                        let image =
                            egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);
                        self.favicons.insert(
                            url.clone(),
                            gui.egui_ctx.load_texture(
                                "favicon",
                                image,
                                egui::TextureOptions::LINEAR,
                            ),
                        );
                    }
                    if width > 0 && height > 0 && pixels.len() == width * height * 4 {
                        let w = width.min(32);
                        let h = height.min(32);
                        let mut rgba = Vec::with_capacity(w * h * 4);
                        for y in 0..h {
                            for x in 0..w {
                                let at = ((y * height / h) * width + x * width / w) * 4;
                                rgba.extend_from_slice(&pixels[at..at + 4]);
                            }
                        }
                        self.apply(AppCommand::Favicon {
                            url,
                            image: profile::SavedFavicon {
                                width: w,
                                height: h,
                                rgba,
                            },
                        });
                        self.changed();
                    }
                    self.window.request_redraw();
                }
                Event::Prompt(prompt) => self.features.prompts.push_back(prompt),
                Event::Status(message) => {
                    eprintln!("{message}");
                    self.status = Some(message);
                }
                Event::Changed => self.changed(),
                Event::Closed(id) => {
                    if let Some(index) = self.session.tabs().iter().position(|t| t.view.id() == id)
                    {
                        self.close_tab(index);
                    }
                }
                Event::Crashed(id, reason) => {
                    if self.features.media_owner == Some(id) {
                        self.features.media_owner = None;
                        self.native_media.clear();
                    }
                    if let Some(tab) = self
                        .session
                        .tabs_mut()
                        .iter_mut()
                        .find(|t| t.view.id() == id)
                    {
                        tab.crash = Some(reason);
                    }
                }
                Event::Popup(request) => {
                    let surface =
                        Rc::new(self.context.offscreen_context(PhysicalSize::new(1000, 700)));
                    let view = request
                        .builder(surface.clone())
                        .user_content_manager(self.content_manager.clone())
                        .delegate(self.delegate.clone())
                        .clipboard_delegate(Rc::new(engine::Clipboard))
                        .hidpi_scale_factor(euclid::Scale::new(self.window.scale_factor() as f32))
                        .build();
                    self.session.push(Tab {
                        view,
                        surface,
                        initial_url: "about:blank".into(),
                        crash: None,
                        history: Default::default(),
                    });
                    self.select_tab(self.session.tabs().len() - 1);
                }
                Event::Permission(origin, request) => self.permissions.push(
                    origin,
                    format!("{:?}", request.feature()),
                    crate::permission_adapter::Permission(request),
                ),
                Event::Authentication(request) => self.authentications.push_back(request),
                Event::Control(id, servo::EmbedderControl::InputMethod(control)) => {
                    let position = control.position();
                    let scale = self.window.scale_factor();
                    self.window.set_ime_cursor_area(
                        PhysicalPosition::new(
                            position.min.x as f64 + self.chrome.page_rect.min.x as f64 * scale,
                            position.min.y as f64 + self.chrome.page_rect.min.y as f64 * scale,
                        ),
                        PhysicalSize::new(
                            position.size().width.max(1) as u32,
                            position.size().height.max(1) as u32,
                        ),
                    );
                    let _ = id;
                }
                Event::Control(id, control) => {
                    self.controls.push_back((id, control));
                    self.window.request_redraw();
                }
                Event::HideControl(id) => self.controls.retain(|(_, control)| control.id() != id),
                Event::Console(id, text) => {
                    let owner = crate::network::owner(id);
                    if !self.tools.debug.contains_key(&owner) {
                        let (level, message) = text.split_once(": ").unwrap_or(("Log", &text));
                        self.tools.log.text(
                            &owner,
                            if matches!(level, "Trace" | "Dir") {
                                "Log"
                            } else {
                                level
                            },
                            "JS",
                            message.into(),
                            serde_json::Value::Null,
                        );
                    }
                    self.console.push_back((id, text));
                    while self.console.len() > 500 {
                        self.console.pop_front();
                    }
                }
                Event::Visited(url, title) => {
                    self.apply(AppCommand::Visit(Bookmark {
                        title,
                        url,
                        visited_at: profile::visit_time(),
                    }));
                }
                Event::Accessibility(id, nodes) => {
                    if let Some(tab) = self.session.tabs().iter().find(|t| t.view.id() == id) {
                        let view = tab.view.clone();
                        let page = self.tools.page(&view);
                        for (id, node) in nodes {
                            if page.ax.len() < 2000 || page.ax.contains_key(&id) {
                                page.ax.insert(id, node);
                            }
                        }
                    }
                }
                Event::Fullscreen(enabled) => {
                    self.fullscreen = enabled;
                    self.window
                        .set_fullscreen(enabled.then_some(Fullscreen::Borderless(None)));
                }
            }
        }
        let url = self.tab().url();
        if url != self.chrome.last_url {
            self.chrome.last_url = url.clone();
            // Do not replace an address while the user is typing.
            let editing = self.gui.as_ref().is_some_and(|g| {
                g.egui_ctx
                    .memory(|m| m.has_focus(egui::Id::unique("address")))
            });
            if !editing {
                self.chrome.address = url;
            }
        }
        self.sync_window_title();
        self.automation_poll();
    }

    fn sync_window_title(&mut self) {
        // Setting an unchanged X11 title emits property events. Doing it on
        // every pump feeds those events back into another title update.
        let title = format!("{} — {}", self.tab().title(), crate::brand::APP_NAME);
        if title != self.last_window_title {
            self.window.set_title(&title);
            self.last_window_title = title;
        }
    }

    pub(crate) fn animating(&self) -> bool {
        self.session.tabs().iter().any(|t| t.view.animating())
    }
    pub(crate) fn smoke_testing(&self) -> bool {
        #[cfg(feature = "test-harness")]
        if self.smoke.is_some()
            || self.download_test.is_some()
            || self.toolbox_test.is_some()
            || self.theme_test.is_some()
            || self.capabilities_test.is_some()
        {
            return true;
        }
        self.capture.is_some()
    }

    pub(crate) fn check_background_tasks(&mut self, event_loop: &ActiveEventLoop) {
        self.check_capture(event_loop);
        #[cfg(feature = "test-harness")]
        self.check_smoke_test(event_loop);
    }

    pub(crate) fn tick(&mut self) {
        self.pump();
        if self.resources.refresh() && !self.fullscreen {
            self.window.request_redraw();
        }
        let repaint_due = {
            let mut deadline = self.repaint_deadline.lock().unwrap();
            if deadline.is_some_and(|when| when <= Instant::now()) {
                *deadline = None;
                true
            } else {
                false
            }
        };
        if repaint_due {
            self.window.request_redraw();
        }
        if self.animating() || self.smoke_testing() || self.tools.pending() {
            self.window.request_redraw();
        }
        if self
            .pending_save
            .is_some_and(|t| t.elapsed() > Duration::from_secs(1))
        {
            self.save();
        }
    }

    fn page_point(&self) -> servo::WebViewPoint {
        let scale = self.window.scale_factor() as f32;
        servo::DevicePoint::new(
            self.chrome.pointer.x as f32 - self.chrome.page_rect.min.x * scale,
            self.chrome.pointer.y as f32 - self.chrome.page_rect.min.y * scale,
        )
        .into()
    }

    fn over_page(&self) -> bool {
        let scale = self.window.scale_factor() as f32;
        self.chrome.page_rect.contains(egui::pos2(
            self.chrome.pointer.x as f32 / scale,
            self.chrome.pointer.y as f32 / scale,
        ))
    }

    fn ui_blocks_pointer(&self) -> bool {
        let Some(gui) = &self.gui else {
            return false;
        };
        let scale = self.window.scale_factor() as f32;
        let point = egui::pos2(
            self.chrome.pointer.x as f32 / scale,
            self.chrome.pointer.y as f32 / scale,
        );
        gui.egui_ctx.egui_is_using_pointer()
            || gui
                .egui_ctx
                .layer_id_at(point)
                .is_some_and(|layer| layer.order != egui::Order::Background)
    }

    pub(crate) fn modal(&self) -> bool {
        self.features.clear_data
            || !self.features.bluetooth.is_empty()
            || !self.features.prompts.is_empty()
            || !self.permissions.is_empty()
            || !self.authentications.is_empty()
            || !self.controls.is_empty()
    }

    fn mouse_button(button: MouseButton) -> servo::MouseButton {
        match button {
            MouseButton::Left => servo::MouseButton::Primary,
            MouseButton::Middle => servo::MouseButton::Auxiliary,
            MouseButton::Right => servo::MouseButton::Secondary,
            MouseButton::Back => servo::MouseButton::Back,
            MouseButton::Forward => servo::MouseButton::Forward,
            MouseButton::Other(n) => servo::MouseButton::Other(n),
        }
    }

    fn release_pointer(&mut self) {
        if self.session.tabs().is_empty() {
            self.chrome.pressed_buttons.clear();
            return;
        }
        for button in std::mem::take(&mut self.chrome.pressed_buttons) {
            self.tab().view.notify_input_event(InputEvent::MouseButton(
                servo::MouseButtonEvent::new(
                    servo::MouseButtonAction::Up,
                    Self::mouse_button(button),
                    self.page_point(),
                ),
            ));
        }
    }

    fn shortcut(&mut self, event: &winit::event::KeyEvent) -> bool {
        let state = self.chrome.modifiers.state();
        let command = if cfg!(target_os = "macos") {
            state.super_key()
        } else {
            state.control_key()
        };
        let pressed = event.state == ElementState::Pressed;
        let character = match &event.logical_key {
            Key::Character(c) => c.to_lowercase(),
            _ => String::new(),
        };
        if pressed
            && command
            && (state.shift_key() || state.alt_key())
            && matches!(character.as_str(), "i" | "e" | "k")
        {
            self.chrome.show_console = true;
            self.tools.active = match character.as_str() {
                "e" => crate::toolbox::Tool::Network,
                "k" => crate::toolbox::Tool::Console,
                _ => crate::toolbox::Tool::Inspector,
            };
            let view = self.tab().view.clone();
            self.tools.refresh(&view, &self.servo);
            self.window.request_redraw();
            return true;
        }
        if pressed && command {
            match character.as_str() {
                "l" if state.shift_key() => self.toggle_layout(),
                "l" => {
                    self.chrome.focus_address = true;
                    self.chrome.page_focused = false;
                    self.tab().view.set_focused(false);
                }
                "t" if state.shift_key() => {
                    if let Some(url) = self.chrome.closed_tabs.pop() {
                        self.add_tab(&url);
                    }
                }
                "t" => {
                    self.add_tab("about:blank");
                    self.chrome.focus_address = true;
                }
                "w" => self.close_tab(self.session.active()),
                "q" => self.quit_requested = true,
                "[" => {
                    self.traverse(false);
                }
                "]" => {
                    self.traverse(true);
                }
                "f" => self.features.find_open = true,
                "r" if state.shift_key() => self.reload_without_cache(),
                "r" => self.reload(),
                "d" => self.bookmark(),
                "h" => {
                    self.chrome.show_history = !self.chrome.show_history;
                    self.chrome.show_bookmarks = false;
                    self.chrome.library_query.clear();
                }
                "b" if state.shift_key() => {
                    self.chrome.show_bookmarks = !self.chrome.show_bookmarks;
                    self.chrome.show_history = false;
                    self.chrome.library_query.clear();
                }
                "+" | "=" => self.zoom(1.1),
                "-" => self.zoom(1.0 / 1.1),
                "0" => self.reset_zoom(),
                n if n.len() == 1 && n.as_bytes()[0].is_ascii_digit() && n != "0" => {
                    let n = character.parse::<usize>().unwrap();
                    self.select_tab(if n == 9 {
                        self.session.tabs().len() - 1
                    } else {
                        (n - 1).min(self.session.tabs().len() - 1)
                    });
                }
                _ => return false,
            }
            self.window.request_redraw();
            return true;
        }
        if pressed {
            if self.debugger_shortcut(&event.logical_key, state.shift_key()) {
                return true;
            }
            match event.logical_key {
                Key::Named(NamedKey::F12) => self.toggle_developer_tools(),
                Key::Named(NamedKey::Escape) if self.cancel_element_picker() => {}
                Key::Named(NamedKey::Escape)
                    if self.chrome.show_console
                        && !self.chrome.page_focused
                        && !self.fullscreen =>
                {
                    self.tools.split_console = !self.tools.split_console;
                }
                Key::Named(NamedKey::F5) => self.reload(),
                Key::Named(NamedKey::Tab) if state.control_key() => {
                    let next = if state.shift_key() {
                        (self.session.active() + self.session.tabs().len() - 1)
                            % self.session.tabs().len()
                    } else {
                        (self.session.active() + 1) % self.session.tabs().len()
                    };
                    self.select_tab(next);
                }
                Key::Named(NamedKey::ArrowLeft) if state.alt_key() => {
                    self.traverse(false);
                }
                Key::Named(NamedKey::ArrowRight) if state.alt_key() => {
                    self.traverse(true);
                }
                Key::Named(NamedKey::Escape) if self.fullscreen => {
                    self.tab().view.exit_fullscreen();
                    self.window.set_fullscreen(None);
                    self.fullscreen = false;
                }
                _ => return false,
            }
            self.window.request_redraw();
            return true;
        }
        false
    }

    pub(crate) fn zoom(&mut self, factor: f32) {
        if let Err(error) =
            self.dispatch_navigation(servo_browser::navigation::Command::Zoom { factor })
        {
            self.status = Some(error);
        }
    }

    pub(crate) fn handle_event(&mut self, event: WindowEvent) {
        if let WindowEvent::ModifiersChanged(mods) = &event {
            self.chrome.modifiers = *mods;
        }
        if let WindowEvent::KeyboardInput { event, .. } = &event {
            if self.shortcut(event) {
                self.pump();
                return;
            }
            if self.chrome.page_focused && !self.modal() {
                input::keyboard(&self.tab().view, event, self.chrome.modifiers);
                self.pump();
                return;
            }
        }
        let response = self
            .gui
            .as_mut()
            .unwrap()
            .on_window_event(&self.window, &event);
        // RedrawRequested already paints below. Scheduling it again would
        // continuously repaint idle windows (egui marks that event repaintable).
        if response.repaint && !matches!(event, WindowEvent::RedrawRequested) {
            self.window.request_redraw();
        }
        match event {
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                self.context.resize(size);
                self.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                for tab in self.session.tabs() {
                    tab.view
                        .set_hidpi_scale_factor(euclid::Scale::new(scale_factor as f32));
                }
                self.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let was_over = self.over_page();
                self.chrome.pointer = position;
                if (self.over_page() || !self.chrome.pressed_buttons.is_empty())
                    && !self.modal()
                    && !self.ui_blocks_pointer()
                {
                    self.tab().view.notify_input_event(InputEvent::MouseMove(
                        servo::MouseMoveEvent::new(self.page_point()),
                    ));
                    self.window
                        .set_cursor(input::cursor(self.tab().view.cursor()));
                } else if was_over {
                    self.tab()
                        .view
                        .notify_input_event(InputEvent::MouseLeftViewport(
                            servo::MouseLeftViewportEvent::default(),
                        ));
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.tab()
                    .view
                    .notify_input_event(InputEvent::MouseLeftViewport(
                        servo::MouseLeftViewportEvent::default(),
                    ));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let up = state == ElementState::Released;
                if (!up && self.over_page() && !self.modal() && !self.ui_blocks_pointer())
                    || (up && self.chrome.pressed_buttons.contains(&button))
                {
                    if up {
                        self.chrome.pressed_buttons.retain(|b| *b != button);
                    } else {
                        self.chrome.pressed_buttons.push(button);
                        self.chrome.page_focused = true;
                        self.gui
                            .as_ref()
                            .unwrap()
                            .egui_ctx
                            .memory_mut(|m| m.surrender_focus(egui::Id::unique("address")));
                        self.tab().view.set_focused(true);
                    }
                    self.tab().view.notify_input_event(InputEvent::MouseButton(
                        servo::MouseButtonEvent::new(
                            if up {
                                servo::MouseButtonAction::Up
                            } else {
                                servo::MouseButtonAction::Down
                            },
                            Self::mouse_button(button),
                            self.page_point(),
                        ),
                    ));
                } else if !up {
                    self.chrome.page_focused = false;
                    self.tab().view.set_focused(false);
                }
            }
            WindowEvent::MouseWheel { delta, .. }
                if self.over_page() && !self.modal() && !self.ui_blocks_pointer() =>
            {
                let (x, y, mode) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        // Servo's compositor consumes these deltas as device pixels.
                        (
                            (x * 76.0) as f64,
                            (y * 76.0) as f64,
                            servo::WheelMode::DeltaLine,
                        )
                    }
                    MouseScrollDelta::PixelDelta(p) => (p.x, p.y, servo::WheelMode::DeltaPixel),
                };
                self.tab()
                    .view
                    .notify_input_event(InputEvent::Wheel(servo::WheelEvent::new(
                        servo::WheelDelta { x, y, z: 0.0, mode },
                        self.page_point(),
                    )));
            }
            WindowEvent::PinchGesture { delta, .. } if self.over_page() && !self.modal() => {
                let scale = self.window.scale_factor() as f32;
                let center = servo::DevicePoint::new(
                    self.chrome.pointer.x as f32 - self.chrome.page_rect.min.x * scale,
                    self.chrome.pointer.y as f32 - self.chrome.page_rect.min.y * scale,
                );
                self.tab()
                    .view
                    .adjust_pinch_zoom((1.0 + delta as f32).max(0.01), center);
                self.window.request_redraw();
            }
            WindowEvent::Ime(event) if self.chrome.page_focused && !self.modal() => {
                let (state, data) = match event {
                    Ime::Preedit(text, _) => {
                        if !self.chrome.ime_composing {
                            self.chrome.ime_composing = true;
                            self.tab().view.notify_input_event(InputEvent::Ime(
                                servo::ImeEvent::Composition(servo::CompositionEvent {
                                    state: servo::CompositionState::Start,
                                    data: String::new(),
                                }),
                            ));
                        }
                        (servo::CompositionState::Update, text)
                    }
                    Ime::Commit(text) => {
                        self.chrome.ime_composing = false;
                        (servo::CompositionState::End, text)
                    }
                    _ => {
                        self.pump();
                        return;
                    }
                };
                self.tab()
                    .view
                    .notify_input_event(InputEvent::Ime(servo::ImeEvent::Composition(
                        servo::CompositionEvent { state, data },
                    )));
            }
            WindowEvent::ThemeChanged(_) => {
                if self.profile().theme == crate::theme::Theme::Glass
                    && let Some(gui) = &self.gui
                {
                    let ctx = gui.egui_ctx.clone();
                    self.set_theme(&ctx, self.profile().theme);
                }
            }
            WindowEvent::Focused(false) => {
                self.release_pointer();
                self.chrome.page_focused = false;
                self.tab().view.set_focused(false);
            }
            _ => {}
        }
        self.pump();
    }

    fn redraw(&mut self) {
        if self.window.inner_size().width == 0 || self.window.inner_size().height == 0 {
            return;
        }
        if let Err(error) = self.context.make_current() {
            self.status = Some(format!("GPU error: {error:?}"));
            return;
        }
        self.pump();
        let mut gui = self.gui.take().unwrap();
        let window = self.window.clone();
        gui.run(&window, |ui| self.ui(ui));
        // Servo may bind its framebuffer while painting. Restore the window target before egui.
        if self.tab().crash.is_none() {
            self.tab().view.paint();
        }
        self.context.prepare_for_rendering();
        gui.painter.clear(
            window.inner_size().into(),
            if self.profile().theme.glass() && self.native_theme.is_some() {
                [0.0, 0.0, 0.0, 0.0]
            } else {
                self.profile()
                    .theme
                    .palette(
                        self.window.theme() != Some(winit::window::Theme::Light),
                        false,
                    )
                    .chrome
                    .to_array()
                    .map(|c| c as f32 / 255.0)
            },
        );
        gui.paint(&window);
        if let Some(capture) = &self.capture
            && capture.ready_for_frame()
        {
            let output = capture.output.with_extension("window.png");
            if !output.exists() {
                let size = window.inner_size();
                let rect = servo::DeviceIntRect::from_size(servo::DeviceIntSize::new(
                    size.width as i32,
                    size.height as i32,
                ));
                if let Some(image) = self.context.read_to_image(rect) {
                    image.save(output).expect("Save browser capture");
                }
            }
        }
        #[cfg(feature = "test-harness")]
        if self.smoke.as_ref().is_some_and(|s| {
            ((8..=10).contains(&s.stage) && s.stage_at.elapsed() > Duration::from_millis(300))
                || (s.stage == 7 && self.profile().layout == TabLayout::Top)
        }) {
            let output = self
                .profile_dir
                .join(if self.profile().layout == TabLayout::Top {
                    "smoke-top-tabs.png"
                } else {
                    match self.smoke.as_ref().unwrap().stage {
                        9 => "smoke-history.png",
                        10 => "smoke-bookmarks.png",
                        _ => "smoke-window.png",
                    }
                });
            if self.profile().layout == TabLayout::Top || !output.exists() {
                let size = window.inner_size();
                let rect = servo::DeviceIntRect::from_size(servo::DeviceIntSize::new(
                    size.width as i32,
                    size.height as i32,
                ));
                if let Some(image) = self.context.read_to_image(rect)
                    && let Err(error) = image.save(output)
                {
                    eprintln!("Cannot save smoke window image: {error}");
                }
            }
        }
        #[cfg(feature = "test-harness")]
        if let Some(test) = &self.toolbox_test
            && let Some(name) = &test.capture
            && !name.starts_with("detached-")
        {
            let size = window.inner_size();
            let rect = servo::DeviceIntRect::from_size(servo::DeviceIntSize::new(
                size.width as i32,
                size.height as i32,
            ));
            if let Some(image) = self.context.read_to_image(rect) {
                image
                    .save(self.profile_dir.join(name))
                    .expect("Toolbox frame export");
            }
        }
        #[cfg(feature = "test-harness")]
        if let Some(test) = &mut self.capabilities_test
            && test.frame_ready()
            && let Some(output) = &test.frame_output
        {
            let size = window.inner_size();
            let rect = servo::DeviceIntRect::from_size(servo::DeviceIntSize::new(
                size.width as i32,
                size.height as i32,
            ));
            if let Some(image) = self.context.read_to_image(rect) {
                image.save(output).expect("Capability UI frame export");
                test.frame_checked = true;
            }
        }
        #[cfg(feature = "test-harness")]
        if let Some(test) = &mut self.theme_test
            && !test.frame_checked
            && let Some(output) = &test.output
        {
            let size = window.inner_size();
            let rect = servo::DeviceIntRect::from_size(servo::DeviceIntSize::new(
                size.width as i32,
                size.height as i32,
            ));
            if let Some(image) = self.context.read_to_image(rect) {
                let scale = window.scale_factor() as f32;
                let chrome = image.get_pixel((5.0 * scale) as u32, (300.0 * scale) as u32)[3];
                let center = self.chrome.page_rect.center();
                let page = image.get_pixel((center.x * scale) as u32, (center.y * scale) as u32)[3];
                let corner = image.get_pixel(
                    (self.chrome.page_rect.min.x * scale) as u32 + 1,
                    (self.chrome.page_rect.min.y * scale) as u32 + 1,
                )[3];
                let glass = self.application.profile().theme.glass() && self.native_theme.is_some();
                if glass {
                    assert!(
                        chrome > 0 && chrome < 255,
                        "Glass chrome has composited alpha: {chrome}"
                    );
                    assert!(
                        corner < 255,
                        "Rounded corners expose the native backdrop: {corner}"
                    );
                } else {
                    assert_eq!(chrome, 255);
                }
                assert_eq!(page, 255, "Web pages remain opaque");
                image.save(output).expect("Save theme frame");
                test.frame_checked = true;
            }
        }
        self.context.present();
        self.gui = Some(gui);
    }

    pub(crate) fn viewport(&mut self, ui: &mut egui::Ui) {
        let outer = ui.available_rect_before_wrap();
        let rect = if self.fullscreen {
            outer
        } else {
            outer.shrink(8.0)
        };
        self.chrome.page_rect = rect;
        if let Some(reason) = self.tab().crash.clone() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("This page stopped responding");
                    ui.label(reason);
                    if ui.button("Reload page").clicked() {
                        let active = self.session.active();
                        self.session.tabs_mut()[active].crash = None;
                        self.reload();
                    }
                });
            });
            return;
        }
        let scale = ui.ctx().pixels_per_point();
        let size = PhysicalSize::new(
            (rect.width() * scale).round().max(1.0) as u32,
            (rect.height() * scale).round().max(1.0) as u32,
        );
        if self.tab().surface.size() != size {
            self.tab().view.resize(size);
        }
        if let Some(blit) = self.tab().surface.render_to_parent_callback() {
            ui.painter().add(egui::PaintCallback {
                rect,
                callback: Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
                    let viewport = info.viewport_in_pixels();
                    blit(
                        painter.gl(),
                        euclid::default::Rect::new(
                            euclid::point2(viewport.left_px, viewport.from_bottom_px),
                            euclid::size2(viewport.width_px, viewport.height_px),
                        ),
                    );
                })),
            });
        }
        if !self.fullscreen {
            crate::ui::page_corners(ui.painter(), rect);
        }
        ui.allocate_rect(outer, egui::Sense::hover());
    }

    pub(crate) fn shutdown(&mut self) {
        self.tool_window.take();
        let _ = self.context.make_current();
        self.save();
        self.tools.shutdown();
        self.permissions.clear();
        self.authentications.clear();
        self.controls.clear();
        self.session.clear();
        self.delegate.events.borrow_mut().clear();
        self.servo.spin_event_loop();
        let _ = self.context.make_current();
        if let Some(gui) = &mut self.gui {
            gui.destroy();
        }
    }
}
