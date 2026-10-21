#[test]
fn chrome_fonts_rasterize_with_servo_features() {
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.label("Lotse Browser — Google HTTPS");
    });
    assert!(!output.shapes.is_empty());
    assert!(
        !output.textures_delta.set.is_empty(),
        "The font atlas must contain rasterized text"
    );
    output.textures_delta.clear();
}
