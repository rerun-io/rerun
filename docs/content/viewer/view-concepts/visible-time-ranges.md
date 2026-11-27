---
title: Visible time ranges
order: 400
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

snippet: howto/visualization/blueprint_view_options[per_entity_time_range]

See [Override path rules](customize-views.md#override-path-rules) for the required entity paths.

You can also set time ranges in the Viewer: select a view or an entity, and edit "Visible time range" in the selection panel.

## Missing or persistent data

See [Why does my data disappear?](../performance/missing-data.md) and [Latest-at semantics](../../log/data-model/latest-at.md) for data that is missing or stays visible.

## Query range and axis window

A visible time range controls which data a view queries, while a plot's axis window controls what part of the time axis is visible.
See [Query range and axis window](../view-types/time-series.md#query-range-and-axis-window).

## Show history in 2D and 3D

### 2D history

By default, a 2D view shows the latest value of each entity at the time cursor.
Set `time_ranges` on `Spatial2DView` to show trails or accumulate data over time.
The 3D history example below also applies to 2D views: replace `Spatial3DView` with `Spatial2DView`.


### 3D trails

Pass a `VisibleTimeRange` to the `time_ranges` parameter of the view:

snippet: archetypes/line_strips3d_time_window

Everything logged in the 5 seconds before the time cursor is now drawn at once.
This is how you draw trails: log one short line segment or one point per time step, and let the time range accumulate them.

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
