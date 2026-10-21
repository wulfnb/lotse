//! AppKit backdrop behind the existing Surfman view; no renderer replacement.
#[cfg(target_os = "macos")]
mod platform {
    use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
    use objc2_app_kit::{
        NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
        NSAutoresizingMaskOptions, NSColor, NSView, NSVisualEffectBlendingMode,
        NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    pub struct NativeTheme {
        effect: Retained<NSVisualEffectView>,
    }
    impl NativeTheme {
        pub fn new(window: &winit::window::Window) -> Result<Self, String> {
            let mtm = MainThreadMarker::new().ok_or("AppKit themes require the main thread")?;
            let RawWindowHandle::AppKit(handle) =
                window.window_handle().map_err(|e| e.to_string())?.as_raw()
            else {
                return Err("Expected an AppKit window".into());
            };
            // Winit owns this NSView for the lifetime of the window. The effect is a
            // sibling below it, so neither mouse handling nor Surfman's layers change.
            let root = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
            let parent = unsafe { root.superview() }.ok_or("The native view has no parent")?;
            let effect =
                NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), root.frame());
            effect.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            effect.setMaterial(NSVisualEffectMaterial::Sidebar);
            effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            effect.setState(NSVisualEffectState::FollowsWindowActiveState);
            effect.setHidden(true);
            parent.addSubview_positioned_relativeTo(
                &effect,
                NSWindowOrderingMode::Below,
                Some(root),
            );
            Ok(Self { effect })
        }
        #[cfg(feature = "test-harness")]
        pub fn glass_active(&self) -> bool {
            !self.effect.isHidden()
                && unsafe { self.effect.superview() }.is_some()
                && self.effect.blendingMode() == NSVisualEffectBlendingMode::BehindWindow
        }
        pub fn apply(&self, theme: crate::theme::Theme, dark: bool) {
            self.effect.setHidden(!theme.glass());
            let name = unsafe {
                if dark {
                    NSAppearanceNameDarkAqua
                } else {
                    NSAppearanceNameAqua
                }
            };
            let appearance = if theme == crate::theme::Theme::Glass {
                None
            } else {
                NSAppearance::appearanceNamed(name)
            };
            self.effect.setAppearance(appearance.as_deref());
            if let Some(window) = self.effect.window() {
                window.setAppearance(appearance.as_deref());
                // Keep the window nonopaque in every theme: Surfman records this
                // at creation. Opaque themes fill the framebuffer themselves.
                window.setOpaque(false);
                window.setBackgroundColor(Some(&NSColor::clearColor()));
            }
        }
    }
    pub fn available_size(window: &winit::window::Window) -> Option<(i32, i32)> {
        let handle = window.window_handle().ok()?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return None;
        };
        let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
        let screen = view.window()?.screen()?;
        let frame = screen.visibleFrame();
        let scale = window.scale_factor();
        Some((
            (frame.size.width * scale).round() as i32,
            (frame.size.height * scale).round() as i32,
        ))
    }
    impl Drop for NativeTheme {
        fn drop(&mut self) {
            self.effect.removeFromSuperview();
        }
    }
}
#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn available_size(_: &winit::window::Window) -> Option<(i32, i32)> {
        None
    }
    pub struct NativeTheme;
    impl NativeTheme {
        pub fn new(_: &winit::window::Window) -> Result<Self, String> {
            Err("Native vibrancy is available on macOS".into())
        }
        pub fn apply(&self, _: crate::theme::Theme, _: bool) {}
        #[cfg(feature = "test-harness")]
        pub fn glass_active(&self) -> bool {
            false
        }
    }
}
pub use platform::{NativeTheme, available_size};
