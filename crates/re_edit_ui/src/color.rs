use re_types::{components::Color, external::arrow2, Loggable};
use re_ui::UiExt;
use re_viewer_context::{
    external::{re_chunk_store::LatestAtQuery, re_entity_db::EntityDb, re_log_types::EntityPath},
    UiLayout, ViewerContext,
};

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

    color_hover(edit_color, response)
}

// TODO(#6661): Should be merged with above method.
pub fn display_color_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _layout: UiLayout,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _path: &EntityPath,
    data: &dyn arrow2::array::Array,
) {
    let color = match Color::from_arrow(data) {
        Ok(color) => color.first().copied(),
        Err(err) => {
            ui.error_label("failed to deserialize")
                .on_hover_text(err.to_string());
            return;
        }
    };

    let Some(color) = color else {
        ui.weak("(none)");
        return;
    };

    let [r, g, b, a] = color.0.to_array();
    let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
    let response = egui::color_picker::show_color(ui, color, egui::Vec2::new(32.0, 16.0));
    ui.painter().rect_stroke(
        response.rect,
        1.0,
        ui.visuals().widgets.noninteractive.fg_stroke,
    );

    color_hover(color, response);
}

fn color_hover(color: egui::Color32, response: egui::Response) -> egui::Response {
    let [r, g, b, a] = color.to_array();
    response.on_hover_ui(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.monospace(format!("#{r:02x}{g:02x}{b:02x}{a:02x}"));
    })
}
