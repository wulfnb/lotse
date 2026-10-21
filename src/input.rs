use servo::{InputEvent, WebView};
use winit::{
    event::{ElementState, KeyEvent, Modifiers},
    keyboard::{Key, KeyLocation, PhysicalKey},
};

pub fn modifiers(mods: Modifiers) -> servo::Modifiers {
    let state = mods.state();
    let mut result = servo::Modifiers::empty();
    result.set(servo::Modifiers::SHIFT, state.shift_key());
    result.set(servo::Modifiers::CONTROL, state.control_key());
    result.set(servo::Modifiers::ALT, state.alt_key());
    result.set(servo::Modifiers::META, state.super_key());
    result
}

pub fn keyboard(view: &WebView, event: &KeyEvent, mods: Modifiers) {
    let key = match &event.logical_key {
        Key::Character(text) => servo::Key::Character(text.to_string()),
        Key::Named(named) => servo::Key::Named(
            format!("{named:?}")
                .parse()
                .unwrap_or(servo::NamedKey::Unidentified),
        ),
        Key::Dead(_) | Key::Unidentified(_) => servo::Key::Named(servo::NamedKey::Unidentified),
    };
    let code = match event.physical_key {
        PhysicalKey::Code(code) => format!("{code:?}")
            .parse()
            .unwrap_or(servo::Code::Unidentified),
        _ => servo::Code::Unidentified,
    };
    let location = match event.location {
        KeyLocation::Left => servo::Location::Left,
        KeyLocation::Right => servo::Location::Right,
        KeyLocation::Numpad => servo::Location::Numpad,
        _ => servo::Location::Standard,
    };
    view.notify_input_event(InputEvent::Keyboard(
        servo::KeyboardEvent::new_without_event(
            if event.state == ElementState::Pressed {
                servo::KeyState::Down
            } else {
                servo::KeyState::Up
            },
            key,
            code,
            location,
            modifiers(mods),
            event.repeat,
            false,
        ),
    ));
}

pub fn cursor(cursor: servo::Cursor) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as C;
    match cursor {
        servo::Cursor::Pointer => C::Pointer,
        servo::Cursor::Text => C::Text,
        servo::Cursor::Crosshair => C::Crosshair,
        servo::Cursor::Move => C::Move,
        servo::Cursor::Wait => C::Wait,
        servo::Cursor::Progress => C::Progress,
        servo::Cursor::NotAllowed => C::NotAllowed,
        servo::Cursor::ColResize => C::ColResize,
        servo::Cursor::RowResize => C::RowResize,
        servo::Cursor::Grab => C::Grab,
        servo::Cursor::Grabbing => C::Grabbing,
        _ => C::Default,
    }
}
