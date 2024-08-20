use re_types::components::ViewCoordinates;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

pub fn edit_or_view_view_coordinates(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, ViewCoordinates>,
) -> egui::Response {
    // Don't allow editing view coordinates for now.
    // It's overrides are likely not always correctly queried.
    UiLayout::List.data_label(ui, value.as_ref().describe())
}
