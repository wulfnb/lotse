use super::*;
#[test]
fn repeats_ignore_timestamps_but_preserve_argument_differences() {
    assert_eq!(
        value_text(&native_value(&devtools_traits::DebuggerValue::NumberValue(
            2.0
        ))),
        "2"
    );
    let mut log = Log::default();
    log.text(
        "tab",
        "Info",
        "JS",
        "same".into(),
        json!({"fields":{"timeStamp":1},"arguments":[{"StringValue":"same"}]}),
    );
    log.text(
        "tab",
        "Info",
        "JS",
        "same".into(),
        json!({"fields":{"timeStamp":2},"arguments":[{"StringValue":"same"}]}),
    );
    assert!(log.entries[0].same_message(&log.entries[1]));
    log.entries[1].details["arguments"] = json!([{"StringValue":"different"}]);
    assert!(!log.entries[0].same_message(&log.entries[1]));
    assert_eq!(
        value_text(&native_value(&devtools_traits::DebuggerValue::NumberValue(
            f64::NAN
        ))),
        "NaN"
    );
}
#[test]
fn filters_history_and_limits() {
    let mut log = Log::default();
    for _ in 0..1002 {
        log.text("tab", "Error", "JS", "real failure".into(), Value::Null);
    }
    assert_eq!(log.entries.len(), 1000);
    let mut state = State {
        query: "failure -ignored".into(),
        ..State::default()
    };
    assert!(state.matches(&log.entries[0]));
    state.levels.remove("Error");
    assert!(!state.matches(&log.entries[0]));
    state.remember("1+2");
    let mut input = "draft".into();
    state.history_move(&mut input, true);
    assert_eq!(input, "1+2");
    state.history_move(&mut input, false);
    assert_eq!(input, "draft");
    assert!(bounded("ééé", 3).starts_with('é'));
    log.preserve = true;
    log.navigation("tab");
    assert_eq!(log.entries.len(), 1000);
    log.clear("tab");
    assert!(log.entries.is_empty());
}

#[test]
fn value_previews_preserve_special_numbers_and_unknown_values() {
    use devtools_traits::DebuggerValue as D;
    let preview: devtools_traits::ObjectPreview = serde_json::from_value(json!({
        "kind":"Map", "entries":[[{"NumberValue":"Infinity"},{"NumberValue":"-Infinity"}]],
        "items":[{"NumberValue":"NaN"}],
        "ownProperties":[{"name":"zero","value":{"NumberValue":-0.0},"configurable":true,"enumerable":true,"writable":true,"isAccessor":false}]
    })).unwrap();
    let value = native_value(&D::ObjectValue {
        actor: Some("map".into()),
        class: "Map".into(),
        own_property_length: Some(1),
        preview: Some(Box::new(preview)),
    });
    assert_eq!(
        value["ObjectValue"]["preview"]["entries"][0][0]["NumberValue"],
        "Infinity"
    );
    assert_eq!(
        value["ObjectValue"]["preview"]["entries"][0][1]["NumberValue"],
        "-Infinity"
    );
    assert_eq!(
        value["ObjectValue"]["preview"]["items"][0]["NumberValue"],
        "NaN"
    );
    assert_eq!(value_text(&value), "Map (1 properties)");
    for (value, expected) in [
        (json!("VoidValue"), "undefined"),
        (json!({"StringValue":17}), ""),
        (json!({"NullValue":true}), "[uninitialized]"),
        (json!({"NullValue":false}), "null"),
        (json!({"BooleanValue":false}), "false"),
        (json!({"NumberValue":-0.0}), "-0"),
        (json!({"ObjectValue":{}}), "Object"),
        (json!({"unknown":true}), "{\"unknown\":true}"),
    ] {
        assert_eq!(value_text(&value), expected);
    }
}

#[test]
fn history_is_bounded_preserves_draft_and_deduplicates_only_neighbors() {
    let mut state = State::default();
    let mut input = "draft".into();
    state.history_move(&mut input, true);
    assert_eq!(input, "draft");
    for i in 0..102 {
        state.remember(&i.to_string());
    }
    state.remember("101");
    assert_eq!(state.history.len(), 100);
    assert_eq!(state.history.front().unwrap(), "2");
    state.history_move(&mut input, true);
    state.history_move(&mut input, true);
    assert_eq!(input, "100");
    state.history_move(&mut input, false);
    assert_eq!(input, "101");
    state.history_move(&mut input, false);
    assert_eq!(input, "draft");
    let mut log = Log::default();
    log.text(
        "a",
        "Log",
        "JS",
        "x".into(),
        json!({"large":"x".repeat(65537)}),
    );
    log.text("b", "Log", "JS", "x".into(), Value::Null);
    assert!(
        log.entries[0].details["note"]
            .as_str()
            .unwrap()
            .contains("64 KiB")
    );
    assert!(!log.entries[0].same_message(&log.entries[1]));
    log.navigation("a");
    assert_eq!(log.entries.len(), 1);
    assert_eq!(log.entries[0].owner, "b");
}
