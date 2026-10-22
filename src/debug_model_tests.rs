use super::*;
fn source(content: Option<String>) -> devtools_traits::SourceInfo {
    serde_json::from_value(serde_json::json!({"url":"https://example.com/a.js","introduction_type":"scriptElement","inline":false,"worker_id":null,"content":content,"content_type":"text/javascript","spidermonkey_id":1})).unwrap()
}
#[test]
fn source_search_selects_document_counts_lines_and_bounds_work() {
    let sources = [
        source(Some("one\nNEEDLE\nthree".into())),
        source(None),
        source(Some("needle".into())),
    ];
    assert!(search(&sources, "", None).0.is_empty());
    let (hits, truncated) = search(&sources, "Needle", Some(0));
    assert!(!truncated);
    assert_eq!(hits.len(), 1);
    assert_eq!((hits[0].source, hits[0].line), (0, 2));
    assert_eq!(hits[0].text, "NEEDLE");
    assert_eq!(search(&sources, "needle", None).0.len(), 2);
    let (hits, truncated) = search(&[source(Some("needle\n".repeat(501)))], "needle", None);
    assert_eq!(hits.len(), 500);
    assert!(truncated);
    assert!(
        search(
            &[source(Some("x".repeat(16 * 1024 * 1024 + 1)))],
            "absent",
            None
        )
        .1
    );
}
