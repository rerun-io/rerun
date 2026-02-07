use re_ui::UiLayout;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn edit_or_view_image_format(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    format: &mut MaybeMutRef<'_, re_sdk_types::components::ImageFormat>,
) -> egui::Response {
    // TODO(#7100): need a ui for editing this!
    UiLayout::List.data_label(
        ui,
        SyntaxHighlightedBuilder::new().with_identifier(&format.as_ref().to_string()),
    )
}
