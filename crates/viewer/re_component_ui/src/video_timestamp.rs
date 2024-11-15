use re_types::components::VideoTimestamp;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn edit_or_view_timestamp(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    timestamp: &mut MaybeMutRef<'_, VideoTimestamp>,
) -> egui::Response {
    let mut timestamp_seconds = timestamp.as_seconds();

    if let Some(timestamp) = timestamp.as_mut() {
        let response = ui.add(
            egui::DragValue::new(&mut timestamp_seconds)
                .clamp_existing_to_range(false)
                .range(0.0..=f32::MAX)
                .speed(0.01) // 0.01 seconds is the smallest step we show right now.
                .custom_formatter(|n, _| re_format::format_timestamp_seconds(n))
                .custom_parser(re_format::parse_timestamp_seconds),
        );

        if response.changed() {
            *timestamp = VideoTimestamp::from_seconds(timestamp_seconds);
        }

        response
    } else {
        ui.label(re_format::format_timestamp_seconds(timestamp_seconds))
    }
    // Show the exact timestamp always in the hover text.
    .on_hover_text(format!("{}ns", re_format::format_int(timestamp.as_nanos())))
}
