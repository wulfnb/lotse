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

#[test]
fn custom_toolbar_controls_expose_names_and_keyboard_actions() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        super::icon(ui, "↻", "Reload page");
    });
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update.unwrap();
    let node = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Reload page"))
        .expect("Accessible toolbar label");
    assert_eq!(node.1.role(), egui::Role::Button);
    assert!(node.1.supports_action(egui::accesskit::Action::Click));
}
