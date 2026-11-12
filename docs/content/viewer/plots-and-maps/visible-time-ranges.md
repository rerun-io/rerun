---
title: Show trails, history, and sliding time windows
order: 250
description: Show trails and a scrolling window of past data in 2D, 3D, and plot views
---

By default, a 2D or 3D view shows the latest value of each entity at the time cursor.
`TimeSeriesView` and `StateTimelineView` show the whole timeline by default.
To show history instead (trails, trajectories, accumulated point clouds, or a scrolling plot window), set a _visible time range_ on the view or on single entities.

Visible time ranges work on these views:

- [`Spatial2DView`](../../reference/types/views/spatial2d_view.md)
- [`Spatial3DView`](../../reference/types/views/spatial3d_view.md)
- [`TimeSeriesView`](../../reference/types/views/time_series_view.md)
- [`StateTimelineView`](../../reference/types/views/state_timeline_view.md)

## Show recent history in a 3D view

Pass a `VisibleTimeRange` to the `time_ranges` parameter of the view:

```python
import rerun as rr
import rerun.blueprint as rrb

rr.send_blueprint(
    rrb.Spatial3DView(
        origin="/",
        time_ranges=rrb.VisibleTimeRange(
            "time",
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-5.0),
            end=rrb.TimeRangeBoundary.cursor_relative(),
        ),
    )
)
```

Everything logged in the 5 seconds before the time cursor is now drawn at once.
This is how you draw trails: log one short line segment or one point per time step, and let the time range accumulate them.

snippet: archetypes/line_strips3d_time_window

## How `VisibleTimeRange` works

`VisibleTimeRange` takes three parameters:

- `timeline`: the timeline that the range applies to.
  It must match the timeline that your data is logged on, for example the name you pass to `rr.set_time`.
  If you only use the built-in timestamps, use `"log_time"` (or `"log_tick"`, if you enabled it with `rr.set_log_tick_enabled(True)` or `RERUN_LOG_TICK=1`).
- `start`: the start of the range.
- `end`: the end of the range.

Set `start` and `end` with a `TimeRangeBoundary`:

- `TimeRangeBoundary.cursor_relative()`: relative to the time cursor.
  Use this for sliding windows.
- `TimeRangeBoundary.absolute()`: a fixed time.
- `TimeRangeBoundary.infinite()`: unbounded.
  For example, `start=rrb.TimeRangeBoundary.infinite()` and `end=rrb.TimeRangeBoundary.cursor_relative()` show all data up to the cursor.

`cursor_relative()` and `absolute()` take one keyword argument that matches the kind of timeline:

- `seconds` or `nanos`: for timelines set with `rr.set_time(…, duration=…)` or `rr.set_time(…, timestamp=…)`.
- `seq`: for timelines set with `rr.set_time(…, sequence=…)`.

You can pass one `VisibleTimeRange` or a list of them, one per timeline.
The range for the active timeline is used.

## Set the time range per entity

To give one entity a different range than the rest of the view, add a `VisibleTimeRanges` override for its entity path:

```python
rrb.Spatial3DView(
    origin="/",
    overrides={
        "/world/robot/trajectory": rrb.VisibleTimeRanges(
            timeline="time",
            start=rrb.TimeRangeBoundary.infinite(),
            end=rrb.TimeRangeBoundary.cursor_relative(),
        ),
    },
)
```

Override paths must be full entity paths.
They do not support `$origin` or wildcards.

You can also set time ranges in the Viewer: select a view or an entity, and edit "Visible time range" in the selection panel.

## Why does old data stay visible?

Views use [latest-at semantics](../../logging-and-ingestion/data-model/latest-at.md): without a time range, each entity shows the last value logged at or before the time cursor.
A value stays visible until you log a new value to the same entity.

To remove an entity from the view at a given time, log a [`Clear`](../../reference/types/archetypes/clear.md) to it:

```python
rr.log("detections", rr.Clear(recursive=False))
```

[Static data](../../logging-and-ingestion/data-model/static.md) is not on any timeline, so it is always visible and is never affected by time ranges.

## Why does my data disappear?

Common reasons:

- The data is logged on a different timeline than the one that is active in the time panel.
  Select the matching timeline in the time panel.
- The time cursor is before the first value of the entity.
- A visible time range is set, and the data is outside of it.

## Scrolling plots and the time axis

In a `TimeSeriesView`, there are two different settings:

- `time_ranges` selects _which data_ the view queries.
  With a cursor-relative range, older data is dropped from the plot.
- `axis_x=rrb.TimeAxis(view_range=…)` sets the _visible window of the X axis_.
  The view only queries data that is inside both this window and `time_ranges`.

This plot shows the last 10 seconds of data and keeps the X axis on that window:

```python
rrb.TimeSeriesView(
    origin="/sensors",
    time_ranges=rrb.VisibleTimeRange(
        "time",
        start=rrb.TimeRangeBoundary.cursor_relative(seconds=-10.0),
        end=rrb.TimeRangeBoundary.cursor_relative(),
    ),
    axis_x=rrb.TimeAxis(
        view_range=rrb.TimeRange(
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-10.0),
            end=rrb.TimeRangeBoundary.cursor_relative(),
        ),
        zoom_lock=True,
    ),
)
```

`zoom_lock=True` keeps the X axis on the window when you zoom.

For a complete scrolling plot, see [Visualize fixed-window plots](fixed-window-plot.md) and the [live scrolling plot example](https://github.com/rerun-io/rerun/tree/latest/examples/python/live_scrolling_plot).

## Link the time axis of several plots

To pan and zoom the time axis of several plots together, set `link` on the `TimeAxis` of each view:

```python
rrb.Vertical(
    rrb.TimeSeriesView(origin="/imu", axis_x=rrb.TimeAxis(link="LinkToGlobal")),
    rrb.TimeSeriesView(origin="/motors", axis_x=rrb.TimeAxis(link="LinkToGlobal")),
)
```

All views with `link="LinkToGlobal"` share one time axis range.
This works for `TimeSeriesView` and the `time_view` of `StateTimelineView`.
The default is `"Independent"`.
