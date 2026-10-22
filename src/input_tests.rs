use super::*;
use winit::keyboard::{KeyCode, ModifiersState, NamedKey, NativeKey, NativeKeyCode};
#[test]
fn keyboard_conversion_preserves_text_state_location_modifiers_and_repeat() {
    for (key, expected) in [
        (
            Key::Character("é".into()),
            servo::Key::Character("é".into()),
        ),
        (
            Key::Named(NamedKey::Enter),
            servo::Key::Named(servo::NamedKey::Enter),
        ),
        (
            Key::Dead(Some('é')),
            servo::Key::Named(servo::NamedKey::Unidentified),
        ),
        (
            Key::Unidentified(NativeKey::Unidentified),
            servo::Key::Named(servo::NamedKey::Unidentified),
        ),
    ] {
        for (location, expected_location) in [
            (KeyLocation::Standard, servo::Location::Standard),
            (KeyLocation::Left, servo::Location::Left),
            (KeyLocation::Right, servo::Location::Right),
            (KeyLocation::Numpad, servo::Location::Numpad),
        ] {
            let event = keyboard_input(
                &key,
                PhysicalKey::Code(KeyCode::KeyA),
                location,
                ElementState::Pressed,
                Modifiers::from(ModifiersState::all()),
                true,
            )
            .event;
            assert_eq!(event.key, expected);
            assert_eq!(event.code, servo::Code::KeyA);
            assert_eq!(event.location, expected_location);
            assert_eq!(event.state, servo::KeyState::Down);
            assert!(event.repeat);
            assert!(!event.is_composing);
            assert!(event.modifiers.contains(
                servo::Modifiers::SHIFT
                    | servo::Modifiers::CONTROL
                    | servo::Modifiers::ALT
                    | servo::Modifiers::META
            ));
        }
    }
    let event = keyboard_input(
        &Key::Dead(None),
        PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
        KeyLocation::Standard,
        ElementState::Released,
        Modifiers::default(),
        false,
    )
    .event;
    assert_eq!(event.code, servo::Code::Unidentified);
    assert_eq!(event.state, servo::KeyState::Up);
    assert!(!event.repeat);
    assert!(event.modifiers.is_empty());
}
#[test]
fn cursor_shapes_cover_supported_page_controls() {
    use winit::window::CursorIcon as C;
    for (source, target) in [
        (servo::Cursor::Pointer, C::Pointer),
        (servo::Cursor::Text, C::Text),
        (servo::Cursor::Crosshair, C::Crosshair),
        (servo::Cursor::Move, C::Move),
        (servo::Cursor::Wait, C::Wait),
        (servo::Cursor::Progress, C::Progress),
        (servo::Cursor::NotAllowed, C::NotAllowed),
        (servo::Cursor::ColResize, C::ColResize),
        (servo::Cursor::RowResize, C::RowResize),
        (servo::Cursor::Grab, C::Grab),
        (servo::Cursor::Grabbing, C::Grabbing),
        (servo::Cursor::Default, C::Default),
    ] {
        assert_eq!(cursor(source), target);
    }
}
