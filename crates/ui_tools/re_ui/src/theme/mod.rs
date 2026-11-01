//! Rerun's theme: the [`StyleProvider`]s that turn widget classes into design tokens.
//!
//! The module is laid out like a stylesheet. [`class`] is the shared vocabulary, and one module per
//! widget holds that widget's rules. The widgets themselves live elsewhere (e.g.
//! [`crate::ButtonExt`]) — they only attach classes, and never decide what a class looks like.
//!
//! [`StyleProvider`]: egui::theme::StyleProvider

pub mod class;

mod button;
mod checkbox;
mod text_edit;

/// Rerun's theme, turning the classes set by [`crate::ButtonExt`] and
/// [`crate::text_edit::ReTextEdit`] into design tokens.
///
/// Widgets that carry no classes get the default variant, so every button and text edit in the
/// viewer is styled by Rerun, whether or not it asked for it.
#[derive(Debug, Clone, Copy)]
pub struct ReTheme;

impl ReTheme {
    /// Make every themed widget in this context use Rerun's styling.
    pub fn install(egui_ctx: &egui::Context) {
        // `replace_` rather than `add_`, so hot-reloading picks up a rebuilt `ReTheme`.
        egui_ctx.replace_widget_theme::<egui::widget_style::ButtonStyle>(Self);
        egui_ctx.replace_widget_theme::<egui::widget_style::TextEditStyle>(Self);
        egui_ctx.replace_widget_theme::<egui::widget_style::CheckboxStyle>(Self);
    }
}
