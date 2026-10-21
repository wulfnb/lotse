use super::*;
#[test]
fn malformed_settings_are_normalized_without_panics() {
    let mut profile = Profile {
        sidebar_width: f32::NAN,
        urls: vec!["javascript://alert(1)".into()],
        active: usize::MAX,
        ..Profile::default()
    };
    profile.favicons.insert(
        "bad".into(),
        SavedFavicon {
            width: usize::MAX,
            height: usize::MAX,
            rgba: vec![],
        },
    );
    profile.normalize();
    assert_eq!(profile.sidebar_width, 240.0);
    assert_eq!(profile.urls, vec![HOME]);
    assert_eq!(profile.active, 0);
    assert!(profile.favicons.is_empty());
    profile.sidebar_width = 9999.0;
    profile.urls = vec![HOME.into(); 200];
    profile.normalize();
    assert_eq!(profile.sidebar_width, 500.0);
    assert_eq!(profile.urls.len(), 100);
}
#[test]
fn bookmark_toggle_preserves_other_entries() {
    let mut profile = Profile::default();
    let initial = profile.bookmarks.len();
    profile.toggle_bookmark("Example".into(), "https://example.com/".into());
    assert_eq!(profile.bookmarks.len(), initial + 1);
    profile.toggle_bookmark("Renamed".into(), "https://example.com/".into());
    assert_eq!(profile.bookmarks.len(), initial);
}

#[test]
fn favicon_restoration_is_bounded_and_keeps_valid_pixels() {
    let mut profile = Profile::default();
    for index in 0..130 {
        profile.favicons.insert(
            format!("{index:03}"),
            SavedFavicon {
                width: 1,
                height: 1,
                rgba: vec![0, 1, 2, 255],
            },
        );
    }
    profile.normalize();
    assert_eq!(profile.favicons.len(), 128);
    assert!(!profile.favicons.contains_key("000"));
    assert_eq!(profile.favicons["129"].rgba, [0, 1, 2, 255]);
}
