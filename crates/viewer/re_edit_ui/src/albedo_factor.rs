use re_types::{components::AlbedoFactor, components::Color};
use re_viewer_context::{MaybeMutRef, ViewerContext};

use crate::color::edit_color_ui;

pub fn edit_albedo_factor_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, AlbedoFactor>,
) -> egui::Response {
    let rgba = value.as_ref().0;

    if let Some(value) = value.as_mut() {
        let mut edit_color = Color(rgba);
        let response = edit_color_ui(ctx, ui, &mut MaybeMutRef::MutRef(&mut edit_color));
        value.0 = edit_color.0;
        response
    } else {
        edit_color_ui(ctx, ui, &mut MaybeMutRef::Ref(&Color(rgba)))
    }
}
