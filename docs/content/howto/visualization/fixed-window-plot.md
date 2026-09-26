---
title: Visualize fixed-window plots
order: 225
description: Configure time series views to show a sliding window
---

The [TimeSeriesView](../../reference/types/views/time_series_view.md) supports direct manipulation of the visible time range.
This allows you to create a plot that only shows a fixed window of data.

## VisibleTimeRange

To specify the visible time range, you must pass one or more `VisibleTimeRange` objects to the `time_ranges` parameter of the `TimeSeriesView` blueprint type. If your app only uses a single timeline, you can directly pass a single `VisibleTimeRange` object instead of wrapping it in a list.

The `VisibleTimeRange` object takes three parameters:
- `timeline`: The timeline that the range will apply to. This must match the timeline used to log your data, or if you are only using the Rerun-provided timestamps, you can use `"log_time"` (or `"log_tick"`, if you enabled it with `rr.set_log_tick_enabled(True)` or `RERUN_LOG_TICK=1`).
- `start`: The start of the visible time range.
- `end`: The end of the visible time range.

The `start` and `end` parameters are set using a `TimeRangeBoundary`:
- To specify an absolute time, you can use the `TimeRangeBoundary.absolute()` method.
- To specify a cursor-relative time, you can use the `TimeRangeBoundary.cursor_relative()` method.
- You can also specify `TimeRangeBoundary.infinite()` to indicate that the start or end of the time range should be unbounded.

In order to account for the different types of timeline (temporal or sequence-based), both the
`TimeRangeBoundary.absolute()` and `TimeRangeBoundary.cursor_relative()` methods can be specified using one of
the keyword args:
- `seconds`/`nanos`: Use these if you called `rr.set_time(…, duration=…)` or `rr.set_time(…, timestamp=…)` to update the timeline.
- `seq`: Use this if you called `rr.set_time(…, sequence=…)` to update the timeline.

## Example syntax
To create a trailing 5 second window plot, you can specify your `TimeSeriesView` like this:
```python
rrb.TimeSeriesView(
    origin="plot_path",
    time_ranges=rrb.VisibleTimeRange(
        timeline="time",
        start=rrb.TimeRangeBoundary.cursor_relative(seconds=-5.0),
        end=rrb.TimeRangeBoundary.cursor_relative(),
    ),
)
```

## Full example
For a complete working example, you can run the following code:

snippet: tutorials/fixed_window_plot

This should create a plot that only shows the last 5 seconds of data. If you select the view, you should
see that the time range is configured as expected.

<picture>
  <img src="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/1200w.png">
</picture>

Alternatively, you can check out a more full-featured example with multiple plot windows [here](https://github.com/rerun-io/rerun/tree/latest/examples/python/live_scrolling_plot).

## Additional notes
- Any time you log data, it is automatically timestamped on the `log_time` timeline (and on `log_tick`, if enabled).
