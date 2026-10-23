//! Launch Services event delivery without replacing winit's application delegate.
use crate::engine::Wake;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSObject, NSObjectProtocol};
use winit::event_loop::EventLoopProxy;

const GET_URL: u32 = u32::from_be_bytes(*b"GURL");
const CORE_EVENT: u32 = u32::from_be_bytes(*b"aevt");
const OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

struct LaunchIvars {
    proxy: EventLoopProxy<Wake>,
}

define_class!(
    // NSObject has no subclass invariants; the handler stays on the main thread.
    #[unsafe(super = NSObject)]
    #[name = "WegaOpenEventHandler"]
    #[thread_kind = MainThreadOnly]
    #[ivars = LaunchIvars]
    struct OpenEventHandler;

    unsafe impl NSObjectProtocol for OpenEventHandler {}

    impl OpenEventHandler {
        #[unsafe(method(handleURL:reply:))]
        fn open_url(&self, event: &NSAppleEventDescriptor, _: &NSAppleEventDescriptor) {
            self.dispatch(event, false);
        }

        #[unsafe(method(handleFiles:reply:))]
        fn open_files(&self, event: &NSAppleEventDescriptor, _: &NSAppleEventDescriptor) {
            self.dispatch(event, true);
        }
    }
);

impl OpenEventHandler {
    fn dispatch(&self, event: &NSAppleEventDescriptor, files: bool) {
        let direct: Option<Retained<NSAppleEventDescriptor>> =
            unsafe { msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT] };
        let Some(direct) = direct else {
            return;
        };
        let count = direct.numberOfItems().clamp(0, 32);
        let items = if count == 0 {
            vec![direct]
        } else {
            (1..=count)
                .filter_map(|index| direct.descriptorAtIndex(index))
                .collect()
        };
        let urls = items
            .iter()
            .filter_map(|value| {
                // Cocoa can coerce an HTTP string into an HFS filesystem path.
                // Only document events contain file descriptors; URL events
                // must retain their original scheme and text.
                if files {
                    value.fileURLValue().and_then(|url| url.absoluteString())
                } else {
                    value.stringValue()
                }
            })
            .filter(|url| url.length() <= 8192)
            .map(|url| url.to_string())
            .collect();
        let _ = self.ivars().proxy.send_event(Wake::OpenUrls(urls));
    }
}

pub(crate) struct OpenEvents {
    handler: Retained<OpenEventHandler>,
}

/// Winit creates NSApplication first; retain handlers until its event loop exits.
pub(crate) fn install(proxy: EventLoopProxy<Wake>) -> OpenEvents {
    let mtm = MainThreadMarker::new().expect("macOS launches on the main thread");
    let handler = OpenEventHandler::alloc(mtm).set_ivars(LaunchIvars { proxy });
    let handler: Retained<OpenEventHandler> = unsafe { msg_send![super(handler), init] };
    let events = OpenEvents { handler };
    events.register();
    events
}

impl OpenEvents {
    /// AppKit installs its document handler during startup. Register again once
    /// the application resumes, while preserving winit's application delegate.
    pub(crate) fn register(&self) {
        let manager = NSAppleEventManager::sharedAppleEventManager();
        let handler = &*self.handler;
        // These public Cocoa selectors take a handler, selector and two FourCharCodes.
        // The typed bindings gate them behind the otherwise unnecessary CoreServices crate.
        unsafe {
            let _: () = msg_send![&*manager, setEventHandler: handler, andSelector: sel!(handleURL:reply:), forEventClass: GET_URL, andEventID: GET_URL];
            let _: () = msg_send![&*manager, setEventHandler: handler, andSelector: sel!(handleFiles:reply:), forEventClass: CORE_EVENT, andEventID: OPEN_DOCUMENTS];
        }
    }
}

impl Drop for OpenEvents {
    fn drop(&mut self) {
        let manager = NSAppleEventManager::sharedAppleEventManager();
        unsafe {
            let _: () =
                msg_send![&*manager, removeEventHandlerForEventClass: GET_URL, andEventID: GET_URL];
            let _: () = msg_send![&*manager, removeEventHandlerForEventClass: CORE_EVENT, andEventID: OPEN_DOCUMENTS];
        }
    }
}
