use re_sdk_types::components::ViewCoordinates2D;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

pub fn edit_or_view_view_coordinates_2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, ViewCoordinates2D>,
) -> egui::Response {
    UiLayout::List.data_label(
        ui,
        SyntaxHighlightedBuilder::new().with_body(&value.as_ref().describe()),
    )
}
