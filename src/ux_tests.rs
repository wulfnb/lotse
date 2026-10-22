use super::*;
#[test]
fn invalid_upload_paths_preserve_a_useful_error() {
    assert!(validate_files("", false).unwrap_err().contains("Choose"));
    assert!(
        validate_files("relative.txt", false)
            .unwrap_err()
            .contains("full path")
    );
    let file = std::env::current_exe().unwrap();
    let input = file.to_str().unwrap();
    assert_eq!(validate_files(input, false).unwrap(), vec![file.clone()]);
    let two = format!("{input}\n{input}");
    assert!(validate_files(&two, false).is_err());
    assert_eq!(validate_files(&two, true).unwrap().len(), 2);
    assert!(
        validate_files(
            &file
                .with_extension("missing-ux-fixture")
                .display()
                .to_string(),
            false
        )
        .is_err()
    );
}
#[test]
fn only_transient_feedback_expires() {
    for severity in [
        Severity::Info,
        Severity::Success,
        Severity::Warning,
        Severity::Error,
    ] {
        let mut notice = Notice::new(severity, "Result", None);
        notice.created = Instant::now() - Duration::from_secs(9);
        assert_eq!(
            notice.expired(),
            matches!(severity, Severity::Info | Severity::Success)
        );
    }
}
