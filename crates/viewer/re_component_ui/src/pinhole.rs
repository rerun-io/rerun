use re_sdk_types::components::PinholeProjection;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

pub fn singleline_view_pinhole(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, PinholeProjection>,
) -> egui::Response {
    // TODO(#6743): Since overrides are not yet taken into account in the transform hierarchy, editing this value has no effect.
    let pinhole = value.as_ref();

    // See if this is a trivial pinhole, and can be displayed as such:
    let fl = pinhole.focal_length_in_pixels();
    let pp = pinhole.principal_point();
    if *pinhole == PinholeProjection::from_focal_length_and_principal_point(fl, pp) {
        let fl = if fl.x() == fl.y() {
            fl.x().to_string()
        } else {
            fl.to_string()
        };

        UiLayout::List.label(ui, format!("Focal length: {fl}, principal point: {pp}"))
    } else {
        UiLayout::List.label(ui, "3×3 projection matrix")
    }
    // TODO(andreas): Make this generic?
    .on_hover_ui(|ui| {
        multiline_view_pinhole(ctx, ui, value);
    })
}

pub fn multiline_view_pinhole(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, PinholeProjection>,
) -> egui::Response {
    // TODO(#6743): Since overrides are not yet taken into account in the transform hierarchy, editing this value has no effect.
    let mat3x3 = value.as_ref().0;

    egui::Grid::new("mat3")
        .num_columns(3)
        .show(ui, |ui| {
            ui.monospace(mat3x3[0].to_string());
            ui.monospace(mat3x3[3].to_string());
            ui.monospace(mat3x3[6].to_string());
            ui.end_row();

            ui.monospace(mat3x3[1].to_string());
            ui.monospace(mat3x3[4].to_string());
            ui.monospace(mat3x3[7].to_string());
            ui.end_row();

            ui.monospace(mat3x3[2].to_string());
            ui.monospace(mat3x3[5].to_string());
            ui.monospace(mat3x3[8].to_string());
            ui.end_row();
        })
        .response
}
