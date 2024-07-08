use re_types::components::Color;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn edit_color_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    color: &mut MaybeMutRef<'_, Color>,
) -> egui::Response {
    let response = if let Some(color) = color.as_mut() {
        let mut edit_color = (*color).into();
        let response = egui::color_picker::color_edit_button_srgba(
            ui,
            &mut edit_color,
            // TODO(#1611): No transparency supported right now.
            // Once we do we probably need to be more explicit about the component semantics.
            egui::color_picker::Alpha::Opaque,
        );
        *color = edit_color.into();
        response
    } else {
        let [r, g, b, a] = color.0.to_array();
        let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
        egui::color_picker::show_color(ui, color, egui::Vec2::new(32.0, 16.0))
    };

    ui.painter().rect_stroke(
        response.rect,
        1.0,
        ui.visuals().widgets.noninteractive.fg_stroke,
    );

    let [r, g, b, a] = color.0.to_array();
    response.on_hover_ui(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.monospace(format!("#{r:02x}{g:02x}{b:02x}{a:02x}"));
    })
}
