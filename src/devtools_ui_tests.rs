#[test]
fn developer_output_keeps_its_monospace_font() {
    let ctx = egui::Context::default();
    crate::ui::style(&ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        super::panel_style(ui);
        ui.monospace("GET /api/data");
    });
    let text = output
        .shapes
        .iter()
        .find_map(|s| {
            if let egui::Shape::Text(text) = &s.shape {
                (text.galley.job.text == "GET /api/data").then_some(text)
            } else {
                None
            }
        })
        .expect("Rendered developer output");
    assert!(
        text.galley
            .job
            .sections
            .iter()
            .all(|s| s.format.font_id.family == egui::FontFamily::Monospace
                && s.format.font_id.size == 12.0)
    );
    output.textures_delta.clear();
}
