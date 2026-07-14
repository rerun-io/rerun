//! Time ruler primitives shared by the time panel and time-aware views.
//!
//! Exposes:
//!
//! - [`TimeRangesUi`] — the linear-piecewise time↔screen mapping used by the
//!   time panel, with optional gap collapsing between linear time segments.
//! - [`paint_time_ranges_and_ticks`] — renders tick marks and labels for one or
//!   more time segments inside a given x-range.
//! - [`paint_time_ranges_gaps`] — fills the regions outside/between the time
//!   segments with a zig-zag-edged dark band, marking the timeline's limits.
//!
//! The crate has no opinion on where the ruler sits on screen or how the user
//! pans and zooms — callers wire those up themselves. The helpers
//! [`TimeRangesUi::pan`] and [`TimeRangesUi::zoom_at`] return new
//! [`TimeView`](re_viewer_context::TimeView)s that the caller
//! can apply.

mod paint_gaps;
mod paint_ticks;
mod time_ranges_ui;

pub use paint_gaps::{MAX_ZIG_WIDTH, paint_time_ranges_gaps};
pub use paint_ticks::paint_time_ranges_and_ticks;
pub use time_ranges_ui::{Segment, TimeRangesUi, gap_width};
