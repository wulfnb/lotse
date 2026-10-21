//! Desktop presentation adapter for core themes.
pub use servo_browser::model::Theme;
pub use servo_browser::theme::{Palette, palette};
pub fn install(ctx: &egui::Context, palette: Palette) {
    servo_browser::theme::install(ctx, palette);
    crate::ui::style(ctx);
}
