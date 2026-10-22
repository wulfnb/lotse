use super::*;

#[test]
fn legacy_internal_links_keep_working_with_lotse_branding() {
    assert_eq!(
        resolve_address("servo://history").unwrap().as_str(),
        "lotse://history"
    );
    assert_eq!(
        resolve_address("lotse://bookmarks").unwrap().as_str(),
        "lotse://bookmarks"
    );
    assert_eq!(
        canonical_internal_address("https://servo.org/"),
        "https://servo.org/"
    );
    let directory =
        std::env::temp_dir().join(format!("lotse-legacy-profile-{}", std::process::id()));
    let original = Profile {
        urls: vec!["https://example.com/".into(), "servo://newtab".into()],
        active: 1,
        ..Profile::default()
    };
    original.save(&directory).unwrap();
    let restored = Profile::load(&directory).unwrap();
    assert_eq!(
        restored.urls,
        vec!["https://example.com/", "lotse://newtab"]
    );
    assert_eq!(restored.active, original.active);
    assert_eq!(restored.bookmarks.len(), original.bookmarks.len());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn addresses_and_encoded_searches() {
    assert_eq!(
        resolve_address("example.com/path").unwrap().as_str(),
        "https://example.com/path"
    );
    assert_eq!(
        resolve_address("localhost:8080/a").unwrap().as_str(),
        "http://localhost:8080/a"
    );
    let url = resolve_address("Rust & Servo + 日本語").unwrap();
    assert_eq!(url.query_pairs().next().unwrap().1, "Rust & Servo + 日本語");
    assert!(resolve_address("javascript://alert(1)").is_err());
}

#[test]
fn old_profiles_keep_undated_history_and_new_visits_round_trip() {
    let mut profile: Profile =
        serde_json::from_str(r#"{"history":[{"title":"Old page","url":"https://example.com/"}]}"#)
            .unwrap();
    assert_eq!(profile.history[0].visited_at, 0);
    assert_eq!(history_date(0, chrono::Local::now()).0, "Earlier visits");
    profile.history[0].visited_at = visit_time();
    let restored: Profile =
        serde_json::from_str(&serde_json::to_string(&profile).unwrap()).unwrap();
    assert_eq!(
        restored.history[0].visited_at,
        profile.history[0].visited_at
    );
    assert_eq!(
        history_date(restored.history[0].visited_at, chrono::Local::now()).0,
        "Today"
    );
}

#[test]
fn reordering_preserves_active_identity() {
    for active in 0..4 {
        for from in 0..4 {
            for to in 0..4 {
                let mut tabs = vec![0, 1, 2, 3];
                let mut selected = active;
                move_tab(&mut tabs, &mut selected, from, to);
                assert_eq!(tabs[selected], active);
            }
        }
    }
}

#[test]
fn profile_round_trip_and_corrupt_file_detection() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "servo-browser-test-{}-{unique}",
        std::process::id()
    ));
    let profile = Profile {
        layout: TabLayout::Top,
        devtools_dock: crate::model::Dock::Right,
        urls: vec!["https://example.com/".into(), "about:blank".into()],
        active: 1,
        ..Profile::default()
    };
    profile.save(&dir).unwrap();
    let restored = Profile::load(&dir).unwrap();
    assert_eq!(restored.layout, TabLayout::Top);
    assert_eq!(restored.devtools_dock, crate::model::Dock::Right);
    assert_eq!(restored.urls, profile.urls);
    assert_eq!(restored.active, 1);
    std::fs::write(dir.join("browser.json"), b"invalid json").unwrap();
    assert!(Profile::load(&dir).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn address_schemes_local_files_and_invalid_hosts() {
    assert_eq!(resolve_address("  ").unwrap().as_str(), HOME);
    #[cfg(unix)]
    assert_eq!(
        resolve_address("/tmp/a b.html").unwrap().as_str(),
        "file:///tmp/a%20b.html"
    );
    #[cfg(windows)]
    assert!(resolve_address("/tmp/a b.html").is_err());
    assert!(resolve_address("about:blank").is_ok());
    assert!(resolve_address("about:config").is_err());
    assert!(resolve_address("https://[").is_err());
    assert!(resolve_address("example..test:invalid-port").is_err());
    assert_eq!(
        resolve_address("[::1]:8080").unwrap().as_str(),
        "http://[::1]:8080/"
    );
    let mut tabs = vec![1, 2];
    let mut active = 1;
    move_tab(&mut tabs, &mut active, 10, 0);
    move_tab(&mut tabs, &mut active, 0, 10);
    assert_eq!(tabs, [1, 2]);
    assert_eq!(active, 1);
    let directory = Profile::default_dir();
    assert!(directory.is_absolute() || directory == Path::new(".servo-browser"));
}

#[test]
fn history_grouping_handles_yesterday_older_and_invalid_timestamps() {
    use chrono::{Local, TimeZone};
    let now = Local
        .with_ymd_and_hms(2026, 10, 21, 12, 0, 0)
        .single()
        .unwrap();
    let yesterday = Local
        .with_ymd_and_hms(2026, 10, 20, 12, 30, 0)
        .single()
        .unwrap();
    let older = Local
        .with_ymd_and_hms(2026, 10, 18, 8, 5, 0)
        .single()
        .unwrap();
    assert_eq!(
        history_date(yesterday.timestamp() as u64, now),
        ("Yesterday".into(), "12:30".into())
    );
    assert_eq!(
        history_date(older.timestamp() as u64, now),
        ("18 Oct 2026".into(), "08:05".into())
    );
    assert_eq!(
        history_date(u64::MAX, now),
        ("Earlier visits".into(), String::new())
    );
    assert_eq!(
        history_date(i64::MAX as u64, now),
        ("Earlier visits".into(), String::new())
    );
}

#[test]
fn malformed_host_ports_are_rejected_before_navigation() {
    assert!(resolve_address("localhost:bad").is_err());
    assert!(resolve_address("example.com:bad").is_err());
}
