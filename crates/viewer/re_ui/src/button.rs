use crate::{DesignTokens, UiExt as _};
use egui::style::WidgetVisuals;
use egui::{Button, CornerRadius, IntoAtoms, Style};

#[derive(Default, Clone, Copy)]
pub enum Variant {
    Primary,
    Secondary,
    #[default]
    Ghost,
    Outlined,
}

pub enum Size {
    Normal,
    Small,
}

impl Size {
    pub fn apply(&self, style: &mut Style) {
        match self {
            Self::Normal => {
                style.spacing.button_padding = egui::vec2(12.0, 8.0);
                all_visuals(style, |vis| {
                    vis.corner_radius = CornerRadius::same(6);
                });
            }
            Self::Small => {
                style.spacing.button_padding = egui::vec2(8.0, 4.0);
                all_visuals(style, |vis| {
                    vis.corner_radius = CornerRadius::same(3);
                });
            }
        }
    }
}

fn all_visuals(style: &mut Style, f: impl Fn(&mut WidgetVisuals)) {
    f(&mut style.visuals.widgets.active);
    f(&mut style.visuals.widgets.hovered);
    f(&mut style.visuals.widgets.inactive);
    f(&mut style.visuals.widgets.noninteractive);
    f(&mut style.visuals.widgets.open);
}

impl Variant {
    pub fn apply(&self, style: &mut Style, tokens: &DesignTokens) {
        match self {
            Self::Primary => {
                all_visuals(style, |vis| {
                    vis.bg_fill = tokens.bg_fill_inverse;
                    vis.weak_bg_fill = tokens.bg_fill_inverse;
                    vis.fg_stroke.color = tokens.text_inverse;
                });
                style.visuals.widgets.hovered.bg_fill = tokens.bg_fill_inverse_hover;
                style.visuals.widgets.hovered.weak_bg_fill = tokens.bg_fill_inverse_hover;
            }
            Self::Secondary => {
                all_visuals(style, |vis| {
                    vis.bg_fill = tokens.widget_active_bg_fill;
                    vis.weak_bg_fill = tokens.widget_active_bg_fill;
                });
                style.visuals.widgets.hovered.bg_fill = tokens.widget_noninteractive_bg_fill;
                style.visuals.widgets.hovered.weak_bg_fill = tokens.widget_noninteractive_bg_fill;
            }
            Self::Ghost => {
                // The default button
            }
            Self::Outlined => {
                all_visuals(style, |vis| {
                    vis.bg_stroke.color = tokens.text_default;
                    vis.bg_stroke.width = 1.0;
                });
            }
        }
    }
}

pub struct ReButton<'a> {
    pub variant: Variant,
    pub size: Size,
    pub inner: Button<'a>,
}

impl<'a> ReButton<'a> {
    pub fn new(atoms: impl IntoAtoms<'a>) -> Self {
        Self::from_button(Button::new(atoms).image_tint_follows_text_color(true))
    }

    pub fn from_button(button: Button<'a>) -> Self {
        ReButton {
            inner: button,
            size: Size::Normal,
            variant: Variant::Ghost,
        }
    }

    pub fn primary(mut self) -> Self {
        self.variant = Variant::Primary;
        self
    }

    pub fn secondary(mut self) -> Self {
        self.variant = Variant::Secondary;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = Variant::Ghost;
        self
    }

    pub fn outlined(mut self) -> Self {
        self.variant = Variant::Outlined;
        self
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn small(mut self) -> Self {
        self.size = Size::Small;
        self
    }

    pub fn normal(mut self) -> Self {
        self.size = Size::Normal;
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

pub trait ReButtonExt<'a> {
    fn primary(self) -> ReButton<'a>;
    fn secondary(self) -> ReButton<'a>;
}

impl<'a> ReButtonExt<'a> for Button<'a> {
    fn primary(self) -> ReButton<'a> {
        ReButton::from_button(self).primary()
    }

    fn secondary(self) -> ReButton<'a> {
        ReButton::from_button(self).secondary()
    }
}

impl egui::Widget for ReButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let previous_style = ui.style().clone();
        let tokens = ui.tokens();
        let style = ui.style_mut();
        self.size.apply(style);
        self.variant.apply(style, tokens);
        let response = ui.add(self.inner);
        ui.set_style(previous_style);
        response
    }
}
