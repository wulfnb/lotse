use super::*;
#[test]
fn host_policy_rejects_lookalikes_and_block_overrides_allow() {
    let policy = Policy {
        blocked_hosts: "ads.example.com".into(),
        allowed_hosts: "*.Example.COM.".into(),
        ..Policy::default()
    };
    for url in ["https://example.com/", "https://sub.example.com/"] {
        assert!(policy.permits(&url::Url::parse(url).unwrap()));
    }
    for url in [
        "https://ads.example.com/",
        "https://child.ads.example.com/",
        "https://example.com.evil/",
        "https://evil-example.com/",
    ] {
        assert!(!policy.permits(&url::Url::parse(url).unwrap()));
    }
    let degenerate = Policy {
        allowed_hosts: "*. .".into(),
        ..Policy::default()
    };
    assert!(!degenerate.permits(&url::Url::parse("data:text/plain,test").unwrap()));
    assert!(!degenerate.permits(&url::Url::parse("https://example.com/").unwrap()));
}

#[test]
fn persisted_theme_names_and_modes_are_compatible() {
    assert_eq!(
        serde_json::from_str::<Theme>("\"Purple\"").unwrap(),
        Theme::Emerald
    );
    for theme in Theme::ALL {
        assert!(!theme.name().is_empty());
        assert_eq!(
            serde_json::from_str::<Theme>(&serde_json::to_string(&theme).unwrap()).unwrap(),
            theme
        );
        assert_eq!(
            theme.glass(),
            matches!(theme, Theme::Glass | Theme::GlassLight | Theme::GlassDark)
        );
    }
    assert!(!Theme::Glass.dark(false));
    assert!(Theme::Glass.dark(true));
    assert!(!Theme::GlassLight.dark(true));
    assert!(!Theme::Paper.dark(true));
    assert!(Theme::Midnight.dark(false));
    assert_eq!(Dock::ALL.len(), 4);
}
