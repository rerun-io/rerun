use crate::re_form::form_strip::FormStrip;
use crate::re_form::{ConstructFormStrip, Fractions};
use crate::{DesignTokens, UiExt as _};
use egui::{Align, Frame, Layout, Margin, Response, Style, Ui, Widget};
use std::ops::{Deref, DerefMut};

/// Wrapper around [`FormStrip`] that applies the expected styling.
pub struct FormFields<'a> {
    strip: FormStrip<'a>,
}

impl<'a> FormFields<'a> {
    pub fn single(ui: &'a mut Ui, widget: impl Widget) -> Response {
        let mut fields = Self::same(ui, 1);
        fields.add(widget)
    }
}

impl<'a> ConstructFormStrip<'a> for FormFields<'a> {
    fn new(ui: &'a mut Ui, fields: Fractions) -> Self {
        let mut strip = FormStrip::new(ui, fields).with_item_layout(
            Layout::left_to_right(Align::Center)
                .with_main_justify(true)
                .with_main_align(Align::Min)
                .with_cross_justify(true),
        );
        let tokens = strip.child_ui.tokens();
        apply_form_field_style(strip.child_ui.style_mut(), tokens);
        Self { strip }
    }
}

impl<'a> Deref for FormFields<'a> {
    type Target = FormStrip<'a>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.strip
    }
}

impl DerefMut for FormFields<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.strip
    }
}

pub fn apply_form_field_style(style: &mut Style, tokens: &DesignTokens) {
    let frame = form_field_frame(tokens);
    style.visuals.widgets.inactive.bg_fill = frame.fill;
    style.visuals.widgets.inactive.weak_bg_fill = frame.fill;
    style.visuals.widgets.active.expansion = 0.0;
    style.visuals.widgets.hovered.expansion = 0.0;
    style.visuals.widgets.open.expansion = 0.0;
    style.spacing.button_padding = frame.inner_margin.left_top();
    style.spacing.item_spacing.x = 4.0;
    style.spacing.item_spacing.y = 4.0;
    style.spacing.interact_size.y = 24.0;
}

pub fn form_field_frame(tokens: &DesignTokens) -> Frame {
    Frame::new()
        .corner_radius(4.0)
        .fill(tokens.form_field_bg_color)
        .inner_margin(Margin::symmetric(6, 0))
}
