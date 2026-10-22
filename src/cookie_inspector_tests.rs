use super::*;
#[test]
fn cookie_poll_distinguishes_pending_success_disconnect_and_timeout() {
    let (tx, rx) = crossbeam_channel::bounded(1);
    let mut state = State {
        pending: Some((rx, Instant::now() + Duration::from_secs(10))),
        ..Default::default()
    };
    state.poll();
    assert!(state.pending());
    assert!(!state.loaded);
    tx.send(vec![]).unwrap();
    state.poll();
    assert!(state.loaded);
    assert!(!state.pending());
    state.poll();
    let (tx, rx) = crossbeam_channel::bounded(1);
    state.pending = Some((rx, Instant::now() + Duration::from_secs(10)));
    drop(tx);
    state.poll();
    assert!(state.error.as_ref().unwrap().contains("cancelled"));
    let (_tx, rx) = crossbeam_channel::bounded(1);
    state.pending = Some((rx, Instant::now() - Duration::from_secs(1)));
    state.poll();
    assert!(state.error.as_ref().unwrap().contains("timed out"));
    assert!(!state.pending());
}
#[test]
fn cookie_table_renders_pending_error_and_empty_filter_states() {
    let ctx = egui::Context::default();
    let (tx, rx) = crossbeam_channel::bounded(1);
    let mut state = State {
        loaded: true,
        error: Some("retry".into()),
        pending: Some((rx, Instant::now())),
        rows: vec![
            Row {
                name: "session".into(),
                value: "private".into(),
                domain: "example.com".into(),
                path: "/".into(),
                expires: "Session".into(),
                same_site: "Lax".into(),
                http_only: true,
                secure: true,
            },
            Row {
                name: "public".into(),
                value: "ok".into(),
                domain: "other.com".into(),
                path: "/".into(),
                expires: "Session".into(),
                same_site: "Unspecified".into(),
                http_only: false,
                secure: false,
            },
        ],
        ..Default::default()
    };
    for filter in ["", "example", "absent"] {
        state.filter = filter.into();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| state.ui(ui));
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }
    drop(tx);
}
