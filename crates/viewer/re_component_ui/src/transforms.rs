use re_sdk_types::components::TransformMat3x3;
use re_sdk_types::datatypes::Mat3x3;
use re_ui::UiExt as _;
use re_viewer_context::MaybeMutRef;

pub fn singleline_view_transform_mat3x3(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, TransformMat3x3>,
) -> egui::Response {
    if value.0 == Mat3x3::IDENTITY {
        ui.label("Identity")
    } else {
        ui.label("3x3 Matrix")
    }
}

pub fn multiline_view_transform_mat3x3(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, TransformMat3x3>,
) -> egui::Response {
    let col0 = value.0.col(0);
    let col1 = value.0.col(1);
    let col2 = value.0.col(2);

    ui.list_item().interactive(false).show_hierarchical(
        ui,
        re_ui::list_item::PropertyContent::new("matrix").value_fn(|ui, _| {
            egui::Grid::new("matrix").num_columns(3).show(ui, |ui| {
                ui.monospace(re_format::format_f32(col0[0]));
                ui.monospace(re_format::format_f32(col1[0]));
                ui.monospace(re_format::format_f32(col2[0]));
                ui.end_row();

                ui.monospace(re_format::format_f32(col0[1]));
                ui.monospace(re_format::format_f32(col1[1]));
                ui.monospace(re_format::format_f32(col2[1]));
                ui.end_row();

                ui.monospace(re_format::format_f32(col0[2]));
                ui.monospace(re_format::format_f32(col1[2]));
                ui.monospace(re_format::format_f32(col2[2]));
                ui.end_row();
            });
        }),
    )
}
