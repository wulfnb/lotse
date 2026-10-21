use super::*;
pub(super) fn start(id: &str, owner: &str) -> Update {
    Update {
        id: id.into(),
        owner: owner.into(),
        at: Instant::now(),
        payload: Payload::Request {
            initial: true,
            url: "https://example.com/data?q=a%26b".into(),
            method: "GET".into(),
            headers: vec![],
            body: Body::default(),
            kind: "Fetch".into(),
            started_ms: 0,
            connect_ms: 0.0,
            send_ms: 0.0,
        },
    }
}
#[test]
fn clear_does_not_resurrect_responses_and_tabs_are_isolated() {
    let mut r = Recorder::default();
    r.apply(start("a", "one"));
    r.apply(start("b", "two"));
    r.clear("one");
    r.apply(Update {
        id: "a".into(),
        owner: "one".into(),
        at: Instant::now(),
        payload: Payload::Response {
            status: 200,
            reason: "OK".into(),
            headers: vec![],
            body: Body::default(),
            cached: false,
        },
    });
    assert!(r.visible("one").is_empty());
    assert_eq!(r.visible("two").len(), 1);
}
#[test]
fn bodies_are_bounded_and_har_is_structured() {
    let body = Body::from_bytes(Some(&vec![b'x'; BODY_LIMIT + 1]));
    assert!(body.truncated());
    assert_eq!(body.bytes.len(), BODY_LIMIT);
    let mut r = Recorder::default();
    r.apply(start("a", "one"));
    let har = r.har("one");
    assert_eq!(
        har["log"]["entries"][0]["request"]["queryString"][0]["value"],
        "a&b"
    );
    assert_eq!(iso_date(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(iso_date(1_704_067_200_123), "2024-01-01T00:00:00.123Z");
}
#[test]
fn filters_sorting_and_request_replay_validation() {
    let mut r = Recorder::default();
    r.apply(start("a", "tab"));
    r.apply(start("b", "tab"));
    r.rows[0].status = Some(404);
    r.rows[0].response_body.size = 100;
    r.rows[1].status = Some(200);
    r.rows[1].response_body.size = 9;
    r.rows[1].kind = "CSS".into();
    r.query = "domain:EXAMPLE.COM method:get -status:404".into();
    assert_eq!(r.visible("tab")[0].id, "b");
    r.types.insert("Fetch".into());
    assert!(r.visible("tab").is_empty());
    r.types.insert("CSS".into());
    assert_eq!(r.visible("tab").len(), 1);
    r.query.clear();
    r.sort = Sort::Size;
    assert_eq!(r.visible("tab")[0].id, "b");
    r.descending = true;
    assert_eq!(r.visible("tab")[0].id, "a");
    let row = &mut r.rows[0];
    row.method = "POST".into();
    row.request_body = Body::from_bytes(Some(b"it's $(literal)"));
    assert!(row.curl().unwrap().contains("'it'\"'\"'s $(literal)'"));
    let mut composer = Composer::from_request(row);
    assert!(composer.script().unwrap().contains("fetch("));
    composer.method = "GET".into();
    assert!(composer.script().is_err());
    composer.body.clear();
    composer.method = "trace".into();
    assert!(composer.script().is_err());
    composer.method = "GET".into();
    composer.url = "file:///tmp/test".into();
    assert!(composer.script().is_err());
    row.request_body.size += 1;
    assert!(row.curl().is_err());
}
#[test]
fn redirect_updates_create_hops_but_late_updates_after_clear_do_not() {
    let mut recorder = Recorder::default();
    recorder.apply(start("redirect", "tab"));
    let mut hop = start("redirect", "tab");
    if let Payload::Request { initial, url, .. } = &mut hop.payload {
        *initial = false;
        *url = "https://example.com/final".into();
    }
    recorder.apply(hop);
    assert_eq!(recorder.visible("tab").len(), 2);
    recorder.query = "final".into();
    assert_eq!(recorder.visible("tab").len(), 1);
    recorder.clear("tab");
    let mut late = start("redirect", "tab");
    if let Payload::Request { initial, .. } = &mut late.payload {
        *initial = false;
    }
    recorder.apply(late);
    assert!(recorder.rows.is_empty());
}
