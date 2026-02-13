use re_sdk_types::components::ViewCoordinates;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

pub fn edit_or_view_view_coordinates(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, ViewCoordinates>,
) -> egui::Response {
    // TODO(#6743): Don't allow editing view coordinates for now.
    // Its overrides are likely not always correctly queried.
    UiLayout::List.data_label(
        ui,
        SyntaxHighlightedBuilder::new().with_body(&value.as_ref().describe()),
    )
}
