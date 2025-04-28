use re_ui::UiLayout;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn edit_or_view_image_format(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    format: &mut MaybeMutRef<'_, re_types::components::ImageFormat>,
) -> egui::Response {
    // TODO(#7100): need a ui for editing this!
    UiLayout::List.data_label(ui, format.as_ref().to_string())
}
