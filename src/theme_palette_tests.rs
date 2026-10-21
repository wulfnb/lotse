use super::*;
#[test]
fn themes_resolve_system_appearance_and_native_fallback() {
    for theme in Theme::ALL {
        let p = theme.palette(false, false);
        assert_eq!(p.chrome.a(), 255);
        assert_ne!(p.text, p.chrome);
    }
    assert!(!Theme::Glass.palette(false, true).dark);
    assert!(Theme::Glass.palette(true, true).dark);
    assert!(Theme::GlassLight.palette(true, true).chrome.a() < 255);
    assert!(!Theme::GlassLight.palette(true, false).glass);
    assert_eq!(
        serde_json::from_str::<crate::profile::Profile>("{}")
            .unwrap()
            .theme,
        Theme::Emerald
    );
}
