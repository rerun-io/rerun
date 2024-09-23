use re_types::components::VideoTimestamp;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub(crate) fn edit_or_view_entity_path(
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
                .custom_formatter(|n, _| format_timestamp_seconds(n))
                .custom_parser(parse_timestamp_seconds),
        );

        if response.changed() {
            *timestamp = VideoTimestamp::from_seconds(timestamp_seconds);
        }

        response
    } else {
        ui.label(format_timestamp_seconds(timestamp_seconds))
    }
}

fn format_timestamp_seconds(timestamp_seconds: f64) -> String {
    let n = timestamp_seconds as i32;
    let hours = n / (60 * 60);
    let mins = (n / 60) % 60;
    let secs_int = n % 60;
    let secs_frac = (timestamp_seconds.fract() * 100.0) as u32;

    if hours > 0 {
        format!("{hours:02}:{mins:02}:{secs_int:02}.{secs_frac:02}")
    } else {
        format!("{mins:02}:{secs_int:02}.{secs_frac:02}")
    }
    // Not showing the minutes at all makes it too unclear what format this timestamp is in.
    // So let's not further strip this down.
}

fn parse_timestamp_seconds(s: &str) -> Option<f64> {
    // For parsing we support:
    // * raw seconds
    // * minutes:seconds
    // * hours:minutes:seconds
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => parts[0].parse::<f64>().ok(),
        2 => {
            let minutes = parts[0].parse::<i32>().ok()?;
            let seconds = parts[1].parse::<f64>().ok()?;
            Some((minutes * 60) as f64 + seconds)
        }
        3 => {
            let hours = parts[0].parse::<i32>().ok()?;
            let minutes = parts[0].parse::<i32>().ok()?;
            let seconds = parts[1].parse::<f64>().ok()?;
            Some(((hours * 60 + minutes) * 60) as f64 + seconds)
        }
        _ => None,
    }
}
