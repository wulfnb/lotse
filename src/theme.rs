//! Desktop presentation adapter for core themes.
pub use wega::model::Theme;
pub use wega::theme::{Palette, palette};
pub fn install(ctx: &egui::Context, palette: Palette) {
    wega::theme::install(ctx, palette);
    crate::ui::style(ctx);
}
