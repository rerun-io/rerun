use egui::widget_style::{CheckboxStyle, StyleArgs, WidgetState};

use crate::ContextExt as _;

/// Rerun's style for every [`egui::Checkbox`]: egui's default, with a thicker checkmark and the
/// `control_fill_*` tokens as fill.
pub struct CheckboxTheme;

impl CheckboxTheme {
    /// Width of the checkmark, in every interaction state.
    const CHECK_STROKE_WIDTH: f32 = 1.5;
}

impl egui::theme::StyleProvider<CheckboxStyle> for CheckboxTheme {
    fn style(&mut self, args: &StyleArgs<'_>) -> CheckboxStyle {
        let mut style: CheckboxStyle = egui::theme::DefaultStyle.style(args);
        style.check_stroke.width = Self::CHECK_STROKE_WIDTH;

        let tokens = args.ctx.tokens();
        match args.state {
            WidgetState::Inactive => style.checkbox_frame.fill = tokens.control_fill,
            WidgetState::Hovered => style.checkbox_frame.fill = tokens.control_fill_hovered,
            WidgetState::Active => style.checkbox_frame.fill = tokens.control_fill_pressed,
            WidgetState::Noninteractive => {}
        }

        style
    }
}
