use re_format::format_f32;
use re_types::components::Resolution;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

pub fn edit_or_view_resolution(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, Resolution>,
) -> egui::Response {
    // Don't allow editing resolution for now.
    let [x, y] = value.as_ref().0 .0;
    UiLayout::List.data_label(ui, format!("{} × {}", format_f32(x), format_f32(y)))
}
