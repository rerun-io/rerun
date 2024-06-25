use re_types::{components::Material, datatypes::Rgba32};
use re_viewer_context::ViewerContext;

use crate::color::edit_color_ui;

// TODO(andreas): as we add more elements to the Material struct, we'll need a multi-line editor.
//                  begs the question though if a material struct is really the right way to go!
pub fn edit_material_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut Material,
) -> egui::Response {
    ui.label("Albedo factor");

    let mut edit_color = value.albedo_factor.unwrap_or(Rgba32::WHITE).into();
    let response = edit_color_ui(ctx, ui, &mut edit_color);
    value.albedo_factor = Some(edit_color.0);

    response
}
