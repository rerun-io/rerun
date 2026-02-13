use re_format::time::{format_relative_timestamp_secs, parse_relative_timestamp_secs};
use re_sdk_types::components::VideoTimestamp;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn edit_or_view_timestamp(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    timestamp: &mut MaybeMutRef<'_, VideoTimestamp>,
) -> egui::Response {
    let mut timestamp_secs = timestamp.as_secs();

    if let Some(timestamp) = timestamp.as_mut() {
        let response = ui.add(
            egui::DragValue::new(&mut timestamp_secs)
                .clamp_existing_to_range(false)
                .range(0.0..=f32::MAX)
                .speed(0.01) // 0.01 seconds is the smallest step we show right now.
                .custom_formatter(|n, _| format_relative_timestamp_secs(n))
                .custom_parser(parse_relative_timestamp_secs),
        );

        if response.changed() {
            *timestamp = VideoTimestamp::from_secs(timestamp_secs);
        }

        response
    } else {
        ui.label(format_relative_timestamp_secs(timestamp_secs))
    }
    // Show the exact timestamp always in the hover text.
    .on_hover_text(format!("{}ns", re_format::format_int(timestamp.as_nanos())))
}
