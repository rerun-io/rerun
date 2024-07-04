use re_types::{components::Material, datatypes::Rgba32};
use re_viewer_context::{MaybeMutRef, ViewerContext};

use crate::color::edit_color_ui;

// TODO(andreas): as we add more elements to the Material struct, we'll need a multi-line editor.
//                  begs the question though if a material struct is really the right way to go!
pub fn edit_material_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, Material>,
) -> egui::Response {
    ui.label("Albedo"); // "Albedo factor" makes the UI too wide.

    let re_types::datatypes::Material { albedo_factor } = value.as_ref().0;
    let albedo_factor = albedo_factor.unwrap_or(Rgba32::WHITE).into();

    if let Some(value) = value.as_mut() {
        let mut edit_color = albedo_factor;
        let response = edit_color_ui(ctx, ui, &mut MaybeMutRef::MutRef(&mut edit_color));
        value.albedo_factor = Some(edit_color.0);
        response
    } else {
        edit_color_ui(ctx, ui, &mut MaybeMutRef::Ref(&albedo_factor))
    }
}
