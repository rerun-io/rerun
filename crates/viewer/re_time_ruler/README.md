# re_time_ruler

Time ruler primitives shared by the time panel and time-aware views.

Exposes:

- [`TimeRangesUi`] — the linear-piecewise time↔screen mapping used by the
  time panel, with optional gap collapsing between linear time segments.
- [`paint_time_ranges_and_ticks`] — renders tick marks and labels for one or
  more time segments inside a given x-range.

The crate has no opinion on how the user pans and zooms — callers wire those up themselves. The helpers
[`TimeRangesUi::pan`] and [`TimeRangesUi::zoom_at`] return new
`TimeView`s that the caller can apply.
