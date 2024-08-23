use re_types::components::ImageFormat;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

pub fn edit_or_view_image_format(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, ImageFormat>,
) -> egui::Response {
    // TODO(#7100): need a ui for editing this!
    let value = value.as_ref();
    let label = if let Some(pixel_format) = value.pixel_format {
        format!("{} {}×{}", pixel_format, value.width, value.height)
    } else {
        format!(
            "{} {} {}×{}",
            value.color_model(),
            value.datatype(),
            value.width,
            value.height
        )
    };
    UiLayout::List.data_label(ui, label)
}
