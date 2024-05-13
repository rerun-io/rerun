---
title: "TimeSeriesView"
---

A time series view.

## Properties

### `ScalarAxis`
Configuration for the scalar axis of a plot.

* range: The range of the axis.
* lock_range_during_zoom: Whether to lock the range of the axis during zoom.
### `PlotLegend`
Configuration for the legend of a plot.

* corner: To what corner the legend is aligned.
* visible: Whether the legend is shown at all.
### `VisibleTimeRanges`
Configures what range of each timeline is shown on a view.

Whenever no visual time range applies, queries are done with "latest at" semantics.
This means that the view will, starting from the time cursor position,
query the latest data available for each component type.

The default visual time range depends on the type of view this property applies to:
- For time series views, the default is to show the entire timeline.
- For any other view, the default is to apply latest-at semantics.

* ranges: The time ranges to show for each timeline unless specified otherwise on a per-entity basis.

## Links
 * 🐍 [Python API docs for `TimeSeriesView`](https://ref.rerun.io/docs/python/stable/common/blueprint_views#rerun.blueprint.views.TimeSeriesView)

## Example

### Use a blueprint to customize a TimeSeriesView.

snippet: views/timeseries

<picture data-inline-viewer="snippets/timeseries">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/timeseries_view/c87150647feb413627fdb8563afe33b39d7dbf57/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/timeseries_view/c87150647feb413627fdb8563afe33b39d7dbf57/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/timeseries_view/c87150647feb413627fdb8563afe33b39d7dbf57/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/timeseries_view/c87150647feb413627fdb8563afe33b39d7dbf57/1200w.png">
  <img src="https://static.rerun.io/timeseries_view/c87150647feb413627fdb8563afe33b39d7dbf57/full.png">
</picture>

