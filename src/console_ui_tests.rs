use super::*;
#[test]
fn structured_previews_expand_without_invoking_accessors_and_stop_at_depth_limit() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    ctx.global_style_mut(|s| s.animation_time = 0.0);
    let values = serde_json::json!({"array":[1,"plain",{"StringValue":"value"},null],"map":{"ObjectValue":{"class":"Map","preview":{"ownProperties":[{"name":"safe","value":{"BooleanValue":true}},{"name":"getter","isAccessor":true}],"items":[{"NullValue":false}],"entries":[[{"StringValue":"key"},{"NumberValue":2}]]}}},"stored":{"ObjectValue":{"class":"Object","actor":"stored"}}});
    let objects = HashMap::from([(
        "stored".into(),
        serde_json::json!({"ObjectValue":{"class":"Object","preview":{"ownProperties":[{"name":"cached","value":{"StringValue":"cached value"}}]}}}),
    )]);
    let mut events = vec![];
    let mut seen = std::collections::HashSet::new();
    let mut text = String::new();
    for _ in 0..12 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: std::mem::take(&mut events),
                ..Default::default()
            },
            |ui| {
                value_ui(ui, "root", &values, &objects, 0);
                value_ui(ui, "deep", &Value::Null, &objects, 8);
            },
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(t) = &shape.shape {
                text.push_str(&t.galley.job.text);
            }
        }
        for (id, node) in &output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
        {
            if node.supports_action(accesskit::Action::Click) && seen.insert(*id) {
                events.push(egui::Event::AccessKitActionRequest(
                    accesskit::ActionRequest {
                        action: accesskit::Action::Click,
                        target_tree: accesskit::TreeId::ROOT,
                        target_node: *id,
                        data: None,
                    },
                ));
            }
        }
        output.textures_delta.clear();
    }
    assert!(text.contains("depth limit"));
    assert!(text.contains("accessor — not invoked"));
    assert!(text.contains("cached value"));
    assert!(text.contains("safe: true"));
}
