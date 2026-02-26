use egui::{
    Color32, Popup, PopupCloseBehavior, Response, Rgba, Ui, Vec2, Widget,
    color_picker::{Alpha, color_picker_hsva_2d},
    epaint::Hsva,
};
use re_sdk_types::datatypes::Rgba32;
use re_ui::UiExt as _;
use re_viewer_context::MaybeMutRef;

/// A simple colored square with rounded corners and a stroke outline.
pub struct ColorSwatch<'a> {
    color: &'a mut MaybeMutRef<'a, Rgba32>,
}

impl<'a> ColorSwatch<'a> {
    /// Create a new [`ColorSwatch`] with the given color.
    pub fn new(color: &'a mut MaybeMutRef<'a, Rgba32>) -> Self {
        Self { color }
    }
}

impl Widget for ColorSwatch<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let [r, g, b, a] = self.color.to_array();
        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
        let egui_color = Color32::from_rgba_unmultiplied(r, g, b, a);

        // Draw the color box.
        let size = Vec2::splat(ui.tokens().color_swatch_size);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        if ui.is_rect_visible(rect) {
            let stroke = if response.hovered() && self.color.as_mut().is_some() {
                ui.tokens().color_swatch_interactive_stroke
            } else {
                ui.tokens().color_swatch_noninteractive_stroke
            };
            ui.painter()
                .rect(rect, 3.0, egui_color, stroke, egui::StrokeKind::Inside);
        }

        // Show the color code on hover.
        let mut response = response.on_hover_ui(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.monospace(format!("#{r:02x}{g:02x}{b:02x}{a:02x}"));
        });

        // Allow editing the color if it's mutable.
        if let Some(color) = self.color.as_mut() {
            let popup_id = ui.auto_id_with("popup");
            const COLOR_SLIDER_WIDTH: f32 = 275.0;
            let mut color_changed = false;
            Popup::menu(&response)
                .id(popup_id)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.spacing_mut().slider_width = COLOR_SLIDER_WIDTH;
                    let mut hsva = Hsva::from(egui_color);

                    if color_picker_hsva_2d(ui, &mut hsva, Alpha::Opaque) {
                        let new_color = Color32::from(Rgba::from(hsva));
                        *color = Rgba32::from(new_color);
                        color_changed = true;
                    }
                });
            if color_changed {
                response.mark_changed();
            }
        }

        response
    }
}
