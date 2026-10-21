//! Engine integration checks; isolated profile and local HTTP fixture only.
use crate::browser::Browser;
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
pub struct Test {
    stage: u8,
    since: Instant,
    start: Instant,
    pending: bool,
    fixture: String,
    pub frame_output: Option<std::path::PathBuf>,
    pub frame_checked: bool,
    result: Rc<RefCell<Option<Result<servo::JSValue, servo::JavaScriptEvaluationError>>>>,
}
impl Test {
    pub fn frame_ready(&self) -> bool {
        self.since.elapsed() > Duration::from_millis(500)
            && !self.frame_checked
            && self.frame_output.is_some()
    }
    pub fn new() -> Self {
        Self {
            stage: 0,
            since: Instant::now(),
            start: Instant::now(),
            pending: false,
            fixture: String::new(),
            frame_output: None,
            frame_checked: false,
            result: Default::default(),
        }
    }
}
impl Browser {
    pub fn check_capabilities_test(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let Some(test) = &self.capabilities_test else {
            return;
        };
        assert!(
            test.start.elapsed() < Duration::from_secs(100),
            "Capabilities test timed out at {}",
            test.stage
        );
        let stage = test.stage;
        if stage == 0 {
            if self.tab().view.load_status() != servo::LoadStatus::Complete
                || self.tab().view.page_title().is_none()
            {
                return;
            }
            let script = Rc::new(servo::UserScript::new(
                "window.__servoUserScript = 73".into(),
                None,
            ));
            self.content_manager.add_script(script.clone());
            self.features.script = Some(script);
            let fixture = self
                .tab()
                .view
                .url()
                .filter(|url| matches!(url.scheme(), "http" | "https"))
                .and_then(|url| url.join("capabilities.html").ok())
                .map(|url| url.to_string())
                .unwrap_or_else(|| "http://127.0.0.1:8767/capabilities.html".into());
            self.capabilities_test.as_mut().unwrap().fixture = fixture.clone();
            self.navigate(&fixture);
            self.feature_stage(1);
            return;
        }
        if test.since.elapsed() < Duration::from_millis(500)
            || self.tab().view.load_status() != servo::LoadStatus::Complete
        {
            return;
        }
        if stage == 1
            && (self.tab().url() != test.fixture
                || self.tab().view.page_title().as_deref() != Some("Capability fixture"))
        {
            return;
        }
        if test.pending {
            let result = test.result.borrow_mut().take();
            let Some(result) = result else { return };
            assert!(
                matches!(result, Ok(servo::JSValue::Boolean(true))),
                "Stage {stage}: {result:?}"
            );
            self.feature_stage(stage + 1);
            return;
        }
        let source=match stage {
            1=>"(()=>{const values={script:window.__servoUserScript,screen:screen.width,width:innerWidth,title:document.title};return values.script===73 && values.screen>=values.width && values.title==='Capability fixture' ? true : JSON.stringify(values)})()".to_owned(),
            2=>{self.tab().view.evaluate_javascript("localStorage.setItem('servo-feature','yes');document.cookie='servo_feature=73;path=/'",|_|{});self.feature_stage(3);return;}
            3=>{self.refresh_site_data();assert!(self.features.cookies.iter().any(|c|c.starts_with("servo_feature=73")),"Engine cookie API");assert!(!self.features.sites.is_empty(),"Engine site data API");let names:Vec<_>=self.features.sites.iter().map(|s|s.0.clone()).collect();let names:Vec<_>=names.iter().map(String::as_str).collect();self.servo.site_data_manager().clear_site_data(&names,servo::StorageType::all());self.feature_stage(4);return;}
            4=>"localStorage.getItem('servo-feature')===null && !document.cookie.includes('servo_feature=')".to_owned(),
            5=>{let find=include_str!("page_actions.js").replace("QUERY","\"needle\"").replace("BACKWARDS","false");format!("({find})==='1 of 2' && window.getSelection().toString()==='needle'")}
            6=>{self.tab().view.evaluate_javascript(include_str!("reader.js"),|_|{});self.feature_stage(7);return;}
            7=>"!!document.getElementById('__servo_reader') && document.getElementById('__servo_reader').textContent.includes('needle')".to_owned(),
            8=>{let fixture=test.fixture.clone();let mut policy=self.profile().policy.clone();policy.offline=true;self.apply(servo_browser::application::Command::Policy(policy));let probe = url::Url::parse(&fixture).unwrap().join("offline-probe").unwrap();self.navigate(probe.as_str());self.feature_stage(9);return;}
            9=>"document.title==='Request blocked' && document.body.textContent.includes('Offline mode')".to_owned(),
            10=>{let mut policy=self.profile().policy.clone();policy.offline=false;self.apply(servo_browser::application::Command::Policy(policy));self.navigate("servo://history");self.feature_stage(11);return;}
            11=>"document.title==='History' && document.querySelector('h1').textContent==='History'".to_owned(),
            12=>{self.screenshot_page();self.feature_stage(13);return;}
            13=>{if !self.profile_dir.join("screenshots").read_dir().unwrap().any(|e|e.unwrap().path().extension().is_some_and(|e|e=="png")){return;} self.tab().view.adjust_pinch_zoom(1.5,servo::DevicePoint::new(100.0,100.0));self.feature_stage(14);return;}
            14=>{assert!(self.tab().view.pinch_zoom()>1.0,"Engine pinch zoom");self.tab().view.clear_session_history();self.save();self.features.settings=true;self.feature_stage(15);self.capabilities_test.as_mut().unwrap().frame_output=Some(self.profile_dir.join("capabilities-settings.png"));return;}
            15=>{if !test.frame_checked{return;}self.features.settings=false;self.refresh_site_data();self.features.site_info=true;self.feature_stage(16);let test=self.capabilities_test.as_mut().unwrap();test.frame_checked=false;test.frame_output=Some(self.profile_dir.join("capabilities-site-info.png"));return;}
            16=>{if !test.frame_checked{return;}println!("CAPABILITIES TEST PASSED: user scripts, screen geometry, engine cookies/site storage/clearing, DOM-range find, reader view, intercepted offline requests, servo://history, asynchronous screenshot and pinch zoom");self.capabilities_test=None;event_loop.exit();return;}
            _=>return,
        };
        let result = self.capabilities_test.as_ref().unwrap().result.clone();
        self.capabilities_test.as_mut().unwrap().pending = true;
        self.tab()
            .view
            .evaluate_javascript(source, move |r| *result.borrow_mut() = Some(r));
    }
    fn feature_stage(&mut self, stage: u8) {
        let test = self.capabilities_test.as_mut().unwrap();
        test.stage = stage;
        test.since = Instant::now();
        test.pending = false;
        println!("CAPABILITIES stage {stage}");
    }
}
