//! How [`egui::Checkbox`] is styled.

use egui::theme::{DefaultStyle, StyleProvider};
use egui::widget_style::{CheckboxStyle, StyleArgs, WidgetState};

use super::ReTheme;
use crate::HasDesignTokens as _;

/// Width of the checkmark, in every interaction state.
const CHECK_STROKE_WIDTH: f32 = 1.5;

/// egui's default checkbox, with a thicker checkmark and the `control_fill_*` tokens as fill.
impl StyleProvider<CheckboxStyle> for ReTheme {
    fn style(&mut self, args: &StyleArgs<'_>) -> CheckboxStyle {
        let mut style: CheckboxStyle = DefaultStyle.style(args);
        style.check_stroke.width = CHECK_STROKE_WIDTH;

        let tokens = args.style.tokens();
        match args.state {
            WidgetState::Inactive => style.checkbox_frame.fill = tokens.control_fill,
            WidgetState::Hovered => style.checkbox_frame.fill = tokens.control_fill_hovered,
            WidgetState::Active => style.checkbox_frame.fill = tokens.control_fill_pressed,
            WidgetState::Noninteractive => {}
        }

        style
    }
}
