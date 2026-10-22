use super::*;
fn row() -> network::Request {
    let mut recorder = network::Recorder::default();
    let value = serde_json::json!({"log":{"entries":[{"startedDateTime":"2026-10-22T00:00:00Z","time":1,"request":{"url":"https://example.com/data","method":"POST","headers":[{"name":"x-request","value":"one"}],"postData":{"text":"payload"}},"response":{"status":200,"statusText":"OK","headers":[{"name":"x-response","value":"two"}],"content":{"text":"reply"}}}]}});
    recorder
        .import_har("tab", &serde_json::to_vec(&value).unwrap())
        .unwrap();
    recorder.rows.pop_front().unwrap()
}
#[test]
fn network_context_actions_copy_exact_values_or_return_explicit_commands() {
    for (label, expected_action, expected_copy) in [
        ("Copy URL", None, Some("https://example.com/data")),
        (
            "Copy as cURL",
            None,
            Some(
                "curl --request 'POST' 'https://example.com/data' --header 'x-request: one' --data-binary 'payload'",
            ),
        ),
        ("Copy request headers", None, Some("x-request: one")),
        ("Copy response headers", None, Some("x-response: two")),
        ("Copy response preview", None, Some("reply")),
        ("Edit and resend…", Some("compose"), None),
        ("Open in new tab", Some("open"), None),
        ("Download file…", Some("download"), None),
        ("Block this domain", Some("block"), None),
    ] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let row = row();
        let mut initial = ctx.run_ui(egui::RawInput::default(), |ui| {
            Browser::network_row_menu(ui, &row);
        });
        let id = initial
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some(label))
            .unwrap_or_else(|| panic!("Missing action {label}"))
            .0;
        initial.textures_delta.clear();
        let mut action = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::AccessKitActionRequest(
                    accesskit::ActionRequest {
                        action: accesskit::Action::Click,
                        target_tree: accesskit::TreeId::ROOT,
                        target_node: id,
                        data: None,
                    },
                )],
                ..Default::default()
            },
            |ui| action = Browser::network_row_menu(ui, &row),
        );
        assert_eq!(action, expected_action, "{label}");
        let copies: Vec<_> = output
            .platform_output
            .commands
            .iter()
            .filter_map(|c| {
                if let egui::OutputCommand::CopyText(text) = c {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            copies,
            expected_copy.into_iter().collect::<Vec<_>>(),
            "{label}"
        );
        output.textures_delta.clear();
    }
}
