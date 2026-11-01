use jiff::Timestamp;
use re_format::{DateVisibility, TimestampFormat, format_plural_s};

use crate::ContextExt as _;

/// Seconds since `timestamp`, negative if it is in the future.
fn elapsed_secs(timestamp: Timestamp) -> f64 {
    Timestamp::now().duration_since(timestamp).as_secs_f64()
}

/// How long ago `timestamp` was, e.g. "just now", "2 minutes ago", or "3 days ago".
///
/// 0-10 seconds: "just now"
/// 10-60 seconds: "less than a minute ago"
/// 1-60 minutes: "X minutes ago"
/// 1-24 hours: "X hours ago"
/// 1-7 days: "X days ago"
/// Over 7 days ago: `None`.
fn try_format_time_ago(timestamp: Timestamp) -> Option<String> {
    let seconds = elapsed_secs(timestamp) as u64;

    let format_plural = |n: u64, unit: &'static str| format!("{} ago", format_plural_s(n, unit));

    Some(if seconds < 10 {
        "just now".to_owned()
    } else if seconds < 60 {
        "less than a minute ago".to_owned()
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        format_plural(minutes, "minute")
    } else if seconds < 24 * 3600 {
        let hours = seconds / 3600;
        format_plural(hours, "hour")
    } else if seconds < 7 * 24 * 3600 {
        let days = seconds / 86400;
        format_plural(days, "day")
    } else {
        return None;
    })
}

/// Like [`try_format_time_ago`], but shows timestamps over 7 days ago with `fallback_format`.
fn format_time_ago(timestamp: Timestamp, fallback_format: TimestampFormat) -> String {
    try_format_time_ago(timestamp).unwrap_or_else(|| fallback_format.format(timestamp))
}

/// Shows a timestamp as a duration from now, in a short format.
///
/// E.g. "1 hour ago", "2 minutes ago", or "just now".
/// Shows the full timestamp on hover.
pub fn short_duration_ui(
    ui: &mut egui::Ui,
    timestamp: Timestamp,
    format: TimestampFormat,
    show: impl FnOnce(&mut egui::Ui, String) -> egui::Response,
) -> egui::Response {
    // Remember to update the ui so it doesn't say "just now" forever:
    let age = elapsed_secs(timestamp);
    let repaint_in_sec = if age < 60.0 {
        1
    } else if age < 3600.0 {
        60
    } else {
        3600
    };
    ui.request_repaint_after(std::time::Duration::from_secs(repaint_in_sec));

    // Snapshot tests always show "just now", whatever the real age is.
    let text = if ui.ctx().is_test() {
        "just now".to_owned()
    } else {
        short_duration_text(timestamp, format)
    };
    let exact = format.format(timestamp);

    // No hover when the label is already the exact timestamp. It would repeat what is on screen.
    let response = show(ui, text.clone());
    if text == exact {
        response
    } else {
        response.on_hover_text(exact)
    }
}

/// The text [`short_duration_ui`] labels a timestamp with.
///
/// Lets a caller tell whether two timestamps get the same label, and leave one of them out when
/// they do.
pub fn short_duration_text(timestamp: Timestamp, format: TimestampFormat) -> String {
    // Past a week `format_time_ago` shows the timestamp instead of a duration. Whole
    // seconds are enough there, since the hover shows the exact one.
    let fallback_format = format
        .with_short(true)
        .with_date_visibility(DateVisibility::ShowDate);

    format_time_ago(timestamp, fallback_format)
}
