use super::*;
#[test]
fn round_trip_binary_fixed_times_and_atomic_errors() {
    let mut r = Recorder::default();
    r.apply(super::super::tests::start("one", "tab"));
    r.rows[0].response_body = Body {
        bytes: vec![0, 255, 128],
        size: 3,
    };
    r.rows[0].status = Some(200);
    r.rows[0].recorded_duration = Some(123.5);
    let har = r.har("tab");
    let mut loaded = Recorder::default();
    assert_eq!(
        loaded
            .import_har("archive", &serde_json::to_vec(&har).unwrap())
            .unwrap(),
        1
    );
    let row = &loaded.rows[0];
    assert!(row.imported);
    assert_eq!(row.response_body.bytes, vec![0, 255, 128]);
    assert_eq!(row.duration_ms(), 123.5);
    let mut bad = har;
    bad["log"]["entries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"request":{"url":"invalid"}}));
    assert!(
        loaded
            .import_har("archive", &serde_json::to_vec(&bad).unwrap())
            .is_err()
    );
    assert_eq!(loaded.rows.len(), 1);
}
