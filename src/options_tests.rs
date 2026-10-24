use super::*;
fn parse(args: &[&str]) -> Result<Options, String> {
    Options::parse_from(args.iter().map(|value| (*value).into()))
}
#[test]
fn every_native_mode_requires_a_separate_profile() {
    for flag in [
        "--smoke-test",
        "--download-test",
        "--toolbox-test",
        "--theme-test",
        "--capabilities-test",
    ] {
        assert!(
            parse(&[flag])
                .unwrap_err()
                .contains("isolated --profile-dir")
        );
        let result = parse(&[flag, "--profile-dir", "/tmp/wega-option-test"]);
        assert_eq!(result.is_ok(), cfg!(feature = "test-harness"));
    }
}
#[test]
fn rejects_invalid_and_missing_arguments_before_startup() {
    for args in [
        vec!["--profile-dir"],
        vec!["--capture"],
        vec!["--remote-devtools"],
        vec!["--webdriver-port"],
        vec!["--webdriver-port", "65536"],
        vec!["--remote-devtools", "abc"],
        vec!["--unknown"],
        vec!["https://a.test", "https://b.test"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    assert_eq!(
        parse(&["--multiprocess"]).is_ok(),
        cfg!(feature = "multiprocess")
    );
}
#[test]
fn normal_startup_and_capture_do_not_require_test_harness() {
    let options = parse(&[
        "--capture",
        "/tmp/capture.png",
        "--webdriver-port",
        "9000",
        "--remote-devtools",
        "9001",
        "https://example.com",
    ])
    .unwrap();
    assert_eq!(options.webdriver_port, Some(9000));
    assert_eq!(options.remote_devtools, Some(9001));
    assert!(!options.test_mode());
    assert_eq!(options.capture, Some(PathBuf::from("/tmp/capture.png")));
}
#[cfg(feature = "test-harness")]
#[test]
fn native_modes_are_mutually_exclusive() {
    assert!(
        parse(&[
            "--smoke-test",
            "--theme-test",
            "--profile-dir",
            "/tmp/wega-option-test"
        ])
        .unwrap_err()
        .contains("only one")
    );
}

#[test]
fn help_is_side_effect_free_and_empty_profile_is_rejected() {
    assert!(Options::parse_from(["--help".into()]).unwrap().help);
    assert!(Options::parse_from(["--profile-dir".into(), "".into()]).is_err());
}

#[test]
fn build_capability_validation_is_independent_of_the_engine() {
    let features = BuildFeatures {
        multiprocess: true,
        test_harness: true,
    };
    let args = [
        "--multiprocess",
        "--download-test",
        "--profile-dir",
        "/tmp/wega-parser",
    ];
    let disabled = BuildFeatures {
        multiprocess: false,
        test_harness: false,
    };
    assert!(
        Options::parse_with(
            ["--download-test", "--profile-dir", "/tmp/wega-parser"].map(String::from),
            disabled
        )
        .unwrap_err()
        .contains("test-harness")
    );
    let options = Options::parse_with(args.map(String::from), features).unwrap();
    assert!(options.multiprocess && options.download_test);
    assert!(
        Options::parse_with(
            [
                "--smoke-test",
                "--theme-test",
                "--profile-dir",
                "/tmp/wega-parser"
            ]
            .map(String::from),
            features
        )
        .unwrap_err()
        .contains("only one")
    );
    let parsed = Options::parse_with(
        ["--profile-dir", "/tmp/p", "--", "-weird-url.html"].map(String::from),
        disabled,
    )
    .unwrap();
    assert_eq!(parsed.url.as_deref(), Some("-weird-url.html"));
    assert!(
        Options::parse_with(
            ["--profile-dir", "/tmp/p", "--", "url1", "url2"].map(String::from),
            disabled,
        )
        .is_err()
    );
}

#[test]
fn os_launch_urls_allow_only_declared_schemes_and_bound_batches() {
    let values = [
        "https://example.test/a?q=1",
        "file:///tmp/some%20page.html",
        "http://localhost:8123/",
        "wega://settings",
        "javascript:alert(1)",
        "data:text/html,x",
        "ftp://example.test/",
        "--profile-dir",
        "not a URL",
    ];
    let input: Vec<_> = values.iter().map(|value| (*value).into()).collect();
    let urls = external_launch_urls(&input);
    assert_eq!(urls.len(), 3);
    assert_eq!(urls[0].as_str(), values[0]);
    assert_eq!(urls[1].as_str(), values[1]);
    let mut input = vec![format!("https://example.test/{}", "x".repeat(8192))];
    input.extend((0..100).map(|number| format!("https://example.test/{number}")));
    let urls = external_launch_urls(&input);
    assert_eq!(urls.len(), 32);
    assert_eq!(urls[0].path(), "/0");
    assert_eq!(urls[31].path(), "/31");
}
