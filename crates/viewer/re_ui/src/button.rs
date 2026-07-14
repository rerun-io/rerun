use crate::{ButtonVisuals, DesignTokens, UiExt as _};
use eframe::emath::Vec2;
use egui::style::WidgetVisuals;
use egui::{
    AtomLayoutResponse, Button, CornerRadius, IntoAtoms, NumExt as _, Rect, Response, Sense, Style,
};

#[derive(Default, Clone, Copy)]
pub enum Variant {
    Primary,
    Secondary,
    #[default]
    Ghost,
    Outlined,

    /// Indicate that the thing this button represents is opened
    Opened,
}

#[derive(Debug, Clone, Copy)]
pub enum Size {
    Normal,
    Small,
    Tiny,
}

impl Size {
    pub fn height(&self) -> f32 {
        match self {
            Self::Normal => 34.0,
            Self::Small => 26.0,
            Self::Tiny => 20.0,
        }
    }

    /// What is the size of the button when it's shown as `icon_button`?
    pub fn icon_button_size(&self) -> Vec2 {
        Vec2::splat(self.height())
    }

    pub fn padding(&self) -> Vec2 {
        let x = match self {
            Self::Normal | Self::Small => 8.0,
            Self::Tiny => 4.0,
        };
        egui::vec2(x, 0.0)
    }

    pub fn apply(&self, style: &mut Style, icon: bool) {
        style.spacing.button_padding = if icon { Vec2::ZERO } else { self.padding() };
        let corner_radius = match self {
            Self::Normal => 6,
            Self::Small | Self::Tiny => 3,
        };
        all_visuals(style, |vis| {
            vis.corner_radius = CornerRadius::same(corner_radius);
        });
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
    fn visuals<'a>(&self, tokens: &'a DesignTokens) -> &'a ButtonVisuals {
        match self {
            Self::Primary => &tokens.button_primary,
            Self::Secondary => &tokens.button_secondary,
            Self::Ghost => &tokens.button_ghost,
            Self::Outlined => &tokens.button_outlined,
            Self::Opened => &tokens.button_opened,
        }
    }

    pub fn apply(&self, style: &mut Style, tokens: &DesignTokens) {
        let visuals = self.visuals(tokens);

        all_visuals(style, |vis| {
            vis.bg_fill = visuals.fill;
            vis.weak_bg_fill = visuals.fill;
            vis.fg_stroke.color = visuals.text;
            vis.bg_stroke = visuals.stroke;
            vis.expansion = 0.0;
        });

        let set_fill = |vis: &mut WidgetVisuals, fill| {
            vis.bg_fill = fill;
            vis.weak_bg_fill = fill;
        };
        set_fill(&mut style.visuals.widgets.hovered, visuals.fill_hovered);
        // `active` is the pressed state; `open` (e.g. an open menu) uses the same fill.
        set_fill(&mut style.visuals.widgets.active, visuals.fill_pressed);
        set_fill(&mut style.visuals.widgets.open, visuals.fill_pressed);
    }
}

pub struct ReButton<'a> {
    pub variant: Variant,
    pub size: Size,
    pub inner: Button<'a>,

    /// If set, the button will be as wide as it is high.
    pub icon: bool,
}

impl<'a> ReButton<'a> {
    pub fn new(atoms: impl IntoAtoms<'a>) -> Self {
        Self::from_button(Button::new(atoms))
    }

    pub fn icon(icon: crate::icons::Icon) -> ReButton<'static> {
        let mut button = ReButton::new(icon);
        button.icon = true;
        button
    }

    pub fn from_button(button: Button<'a>) -> Self {
        ReButton {
            inner: button.image_tint_follows_text_color(true),
            size: Size::Normal,
            variant: Variant::Ghost,
            icon: false,
        }
    }

    pub fn image_tint_follows_text_color(mut self, follows: bool) -> Self {
        self.inner = self.inner.image_tint_follows_text_color(follows);
        self
    }

    pub fn primary(mut self) -> Self {
        self.variant = Variant::Primary;
        self
    }

    pub fn secondary(mut self) -> Self {
        self.variant = Variant::Secondary;
        self
    }

    pub fn opened(mut self) -> Self {
        self.variant = Variant::Opened;
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

    pub fn tiny(mut self) -> Self {
        self.size = Size::Tiny;
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

    /// Show a [`Button`] that reveals more icon buttons (or other content) on hover.
    ///
    /// `button` will be called multiple times on frames where the button is hovered.
    ///
    /// Pass in the width the hover ui will need (usually this follows the formula
    /// `count * button_width + (count - 1) * icon_spacing`)
    pub fn with_hover_icon_buttons<R>(
        ui: &mut egui::Ui,
        mut button: impl FnMut() -> Self,
        mut hover_buttons_width: f32,
        hover_buttons: impl FnOnce(&mut egui::Ui) -> R,
    ) -> (AtomLayoutResponse, Option<R>) {
        // Left and right spacing around the icons + some tolerance
        hover_buttons_width += ui.spacing().icon_spacing * 2.0 + 1.0;

        let clip_rect = ui.clip_rect();

        let calc_rect_with_buttons = |mut button_rect: Rect, available_rect: Rect| {
            let limit = available_rect
                .max
                .x
                .at_least(button_rect.max.x)
                .at_most(clip_rect.max.x);
            button_rect.max.x = (button_rect.max.x + hover_buttons_width).at_most(limit);
            button_rect
        };

        let id = ui.next_auto_id();
        let hovered = ui.read_response(id).is_some_and(|last| {
            let rect_with_buttons =
                calc_rect_with_buttons(last.interact_rect, ui.available_rect_before_wrap());
            ui.rect_contains_pointer(rect_with_buttons)
        });

        let mut atom_response = None;
        ui.add_visible(!hovered, |ui: &mut egui::Ui| {
            let atom_layout_response = button().atom_ui(ui);
            atom_response = Some(atom_layout_response.clone());
            atom_layout_response.response
        });
        let response = atom_response.expect("Should be set now");

        // Due to the interact_radius there would be a couple px where the cursor would trigger the
        // hover without actually being rect_contains_pointer, which looks confusing (hovered but
        // the icon button isn't shown). To mask, interact on top of the button so it doesn't get
        // the hover style:
        ui.interact(response.rect, id.with("hover_mask"), Sense::click());

        if !hovered {
            ui.skip_ahead_auto_ids(1);
            return (response, None);
        }

        let rect_with_buttons =
            calc_rect_with_buttons(response.rect, ui.available_rect_before_wrap());

        let mut extra_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect_with_buttons)
                .layout(egui::Layout::right_to_left(egui::Align::Min)),
        );
        extra_ui.spacing_mut().item_spacing.x = ui.spacing().icon_spacing;

        let mut button = button();
        button.inner = button.inner.truncate();

        let (response, icon_response) = egui::Sides::new()
            .spacing(ui.spacing().icon_spacing)
            .shrink_left()
            .show(
                &mut extra_ui,
                |ui| button.atom_ui(ui),
                |ui| {
                    ui.add_space(ui.spacing().icon_spacing);
                    hover_buttons(ui)
                },
            );

        (response, Some(icon_response))
    }

    /// Show a [`Button`] that reveals an icon button on hover.
    ///
    /// `button` will be called multiple times on frames where the button is hovered.
    pub fn with_hover_icon_button(
        ui: &mut egui::Ui,
        icon: ReButton<'static>,
        mut button: impl FnMut() -> Self,
    ) -> (AtomLayoutResponse, Option<Response>) {
        let size = button().size;
        let icon = icon.size(size);
        Self::with_hover_icon_buttons(ui, button, size.icon_button_size().x, |ui| ui.add(icon))
    }

    pub fn atom_ui(self, ui: &mut egui::Ui) -> AtomLayoutResponse {
        let previous_style = ui.style().clone();
        let tokens = ui.tokens();
        let style = ui.style_mut();
        self.size.apply(style, self.icon);
        self.variant.apply(style, tokens);
        let response = self
            .inner
            .min_size(self.size.icon_button_size())
            .atom_ui(ui);
        ui.set_style(previous_style);
        response
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
        self.atom_ui(ui).response
    }
}
