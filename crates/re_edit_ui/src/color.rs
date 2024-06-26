use re_types::components::Color;
use re_viewer_context::ViewerContext;

pub fn edit_color_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut Color,
) -> egui::Response {
    let mut edit_color = (*value).into();
    let response = egui::color_picker::color_edit_button_srgba(
        ui,
        &mut edit_color,
        // TODO(#1611): No transparency supported right now.
        // Once we do we probably need to be more explicit about the component semantics.
        egui::color_picker::Alpha::Opaque,
    );
    *value = edit_color.into();

    ui.painter().rect_stroke(
        response.rect,
        1.0,
        ui.visuals().widgets.noninteractive.fg_stroke,
    );

    let [r, g, b, a] = edit_color.to_array();
    response.on_hover_ui(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.monospace(format!("#{r:02x}{g:02x}{b:02x}{a:02x}"));
    })
}
