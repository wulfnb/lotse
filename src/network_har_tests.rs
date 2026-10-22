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

fn archive() -> Value {
    let mut r = Recorder::default();
    r.apply(super::super::tests::start("one", "tab"));
    r.rows[0].status = Some(200);
    r.har("tab")
}
#[test]
fn malformed_har_fields_are_rejected_atomically() {
    let original = archive();
    for (pointer, value) in [
        ("/log/entries/0/request/headers", json!(false)),
        (
            "/log/entries/0/request/headers",
            json!([{"name":0,"value":"x"}]),
        ),
        (
            "/log/entries/0/request/headers",
            json!([{"name":"x","value":0}]),
        ),
        (
            "/log/entries/0/request/headers",
            json!([{"name":"x".repeat(1025),"value":"x"}]),
        ),
        (
            "/log/entries/0/request/headers",
            json!(vec![json!({"name":"x","value":"y"}); 201]),
        ),
        ("/log/entries/0/request/url", json!("x".repeat(65537))),
        ("/log/entries/0/request/method", json!("bad method")),
        ("/log/entries/0/time", json!(-1)),
        (
            "/log/entries/0/startedDateTime",
            json!("1969-12-31T23:59:59Z"),
        ),
        ("/log/entries/0/response/status", json!(600)),
        (
            "/log/entries/0/response/statusText",
            json!("x".repeat(8193)),
        ),
    ] {
        let mut bad = original.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        let mut r = Recorder::default();
        assert!(
            r.import_har("tab", &serde_json::to_vec(&bad).unwrap())
                .is_err(),
            "{pointer}"
        );
        assert!(r.rows.is_empty());
    }
    assert!(import(&mut Recorder::default(), "tab", &vec![b' '; FILE_LIMIT + 1]).is_err());
    let mut bad = original.clone();
    bad["log"]["entries"] = json!(vec![original["log"]["entries"][0].clone(); ROW_LIMIT + 1]);
    assert!(
        import(
            &mut Recorder::default(),
            "tab",
            &serde_json::to_vec(&bad).unwrap()
        )
        .is_err()
    );
    let mut bad = original.clone();
    bad["log"]["entries"][0]["_initiator"] = json!({"type":"x".repeat(1025)});
    assert!(
        import(
            &mut Recorder::default(),
            "tab",
            &serde_json::to_vec(&bad).unwrap()
        )
        .is_err()
    );
    assert!(body(&json!({"encoding":"base64","text":"!!!"}), None).is_err());
    assert_eq!(
        headers(&Value::Null).unwrap(),
        Vec::<(String, String)>::new()
    );
}

#[test]
fn imports_classify_resources_bound_previews_and_evict_old_rows() {
    for (resource, mime, expected) in [
        ("document", "", "HTML"),
        ("script", "", "JS"),
        ("stylesheet", "", "CSS"),
        ("xhr", "", "XHR"),
        ("fetch", "", "Fetch"),
        ("image", "", "Img"),
        ("font", "", "Font"),
        ("media", "", "Media"),
        ("websocket", "", "WS"),
        ("", "text/html", "HTML"),
        ("", "text/css", "CSS"),
        ("", "application/javascript", "JS"),
        ("", "image/png", "Img"),
        ("", "font/woff2", "Font"),
        ("", "audio/wav", "Media"),
        ("", "video/webm", "Media"),
        ("", "text/plain", "Other"),
    ] {
        let mut value = archive();
        let e = &mut value["log"]["entries"][0];
        e["_resourceType"] = json!(resource);
        e["response"]["content"]["mimeType"] = json!(mime);
        let mut r = Recorder::default();
        assert_eq!(
            import(&mut r, "archive", &serde_json::to_vec(&value).unwrap()).unwrap(),
            1
        );
        assert_eq!(r.rows[0].kind, expected);
    }
    let bytes = "x".repeat(BODY_LIMIT + 1);
    let preview = body(&json!({"text":bytes}), None).unwrap();
    assert!(preview.truncated());
    assert!(content(&preview, "")["comment"].is_string());
    let mut value = archive();
    value["log"]["entries"] = json!(vec![value["log"]["entries"][0].clone(); ROW_LIMIT]);
    let mut r = Recorder::default();
    r.apply(super::super::tests::start("old", "tab"));
    import(&mut r, "archive", &serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(r.rows.len(), ROW_LIMIT);
    assert!(r.rows.iter().all(|row| row.imported));
}

#[test]
fn corrupt_and_incomplete_har_envelopes_are_explicit_errors() {
    for bytes in [b"{".as_slice(), b"{}", b"{\"log\":{\"entries\":false}}"] {
        let mut recorder = Recorder::default();
        assert!(import(&mut recorder, "archive", bytes).is_err());
        assert!(recorder.rows.is_empty());
    }
    assert!(
        bounded(&json!({}), "url", 100)
            .unwrap_err()
            .contains("must be a string")
    );
}
