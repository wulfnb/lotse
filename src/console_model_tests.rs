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
