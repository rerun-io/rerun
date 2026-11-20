---
title: Time series
order: 30
description: Compare numeric values over time
---

Use a time series view to plot numeric signals over time.
It is useful for spotting trends, spikes, and differences between signals, such as sensor readings, model scores, or tracking errors.

See the [`TimeSeriesView` reference](../../reference/types/views/time_series_view.md) for properties and examples.

## Plot any scalar

Rerun can plot numerical data as a time series, even data that wasn't logged with Rerun semantics.
By remapping where a visualizer reads its inputs from, you can separate how you _model_ your data from how you _visualize_ it.
This is useful for plotting custom messages from MCAPs, or data logged via `AnyValues` and `DynamicArchetype`.
Logging multiple scalars to the same entity also makes `.rrd` files smaller than logging each scalar to its own entity.

Each visualizer takes components as input and determines their values from various sources.
By configuring _component mappings_, you can control exactly where each input comes from.

The supported data types are:

- `Float32` and `Float64`
- `Int8`, `Int16`, `Int32`, and `Int64`
- `UInt8`, `UInt16`, `UInt32`, and `UInt64`
- `Boolean`
- Any of the above nested inside of [Arrow structs](https://arrow.apache.org/docs/format/Intro.html#struct).

For background on how visualizers resolve component values, see [Customize views](../view-concepts/customize-views.md).
For styling and axes, see [Customize plots](time-series.md#customize-plots).

### Logging custom data

See [Logging custom data](../../logging-and-ingestion/send-data/custom-data.md#logging-custom-data) for the shared setup and scalar example.

### Remapping components

A visualizer can source its inputs from any component with a compatible datatype.
For example, the `SeriesLines` visualizer accepts any numerical data for its `Scalar` input.
This works with data from MCAP files, `AnyValues`, or `DynamicArchetype`.
Optional components like `Names` and `Colors` can be sourced similarly from arbitrary data.

The following remaps the `Scalars:scalars` input to read from `custom:my_custom_scalar` instead:

snippet: howto/component_mapping[source_mapping]

#### Add data by dragging components

You can set up this mapping interactively instead of via the blueprint API: drag a component from the streams tree onto a time series view.
If the component has a compatible (numeric) datatype, a new `SeriesLines` visualizer is added that remaps `Scalars:scalars` from it.
Non-numeric components (e.g. a string) are rejected, as is dropping a component that the view already plots.

### Selectors for nested data

See [Selectors for nested data](../view-concepts/component-mappings.md#selectors-for-nested-data) for the shared setup and scalar example.

### Providing default values

See [Providing default values](../view-concepts/component-mappings.md#providing-default-values) for the shared setup and scalar example.

### Complete scalar mapping example

The complete example logs three series to a single entity and configures each with a different component mapping strategy.
This leads to the following visualizers for the `/plot` entity:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mapping-all-visualizers/82b4ea0a8290bb3b5043296cae810002c1bd5174/full.png" alt="Visualizer configuration for the /plot entity after remapping components">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mapping-all-visualizers/82b4ea0a8290bb3b5043296cae810002c1bd5174/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mapping-all-visualizers/82b4ea0a8290bb3b5043296cae810002c1bd5174/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mapping-all-visualizers/82b4ea0a8290bb3b5043296cae810002c1bd5174/1024w.png">
</picture>

* 🐍 [Python](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/howto/component_mapping.py)
* 🦀 [Rust](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/howto/component_mapping.rs)

<picture>
  <img src="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/full.png" alt="Three series plotted from a single entity using different component mapping strategies">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/1200w.png">
</picture>

When the view is selected, the selection panel shows an overview of all configured visualizers:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mapping-visualizer-list/48e05924eac3a4a93847299707132c73fe10609c/full.png" alt="Visualizer list in the selection panel showing the three configured series">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mapping-visualizer-list/48e05924eac3a4a93847299707132c73fe10609c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mapping-visualizer-list/48e05924eac3a4a93847299707132c73fe10609c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mapping-visualizer-list/48e05924eac3a4a93847299707132c73fe10609c/1024w.png">
</picture>

## Show a subset of plotted series

When you log several values in one [`Scalars`](../../reference/types/archetypes/scalars.md) batch, such as the joint positions of a robot arm, each value is plotted as its own series in a [TimeSeriesView](../../reference/types/views/time_series_view.md).
You can show only some of them from the Viewer or from code.

### In the Viewer

Click a series in the plot legend to hide or show it.
The Viewer stores this choice in the blueprint.

### From code

Set `visible_series` of [`SeriesLines`](../../reference/types/archetypes/series_lines.md) as a blueprint override.
It takes one boolean per series, in the order of the values in the batch.
If the list is shorter than the number of series, its last value is repeated.
Hidden series stay in the legend, so you can click them to show them again.
For series drawn as points, use [`SeriesPoints`](../../reference/types/archetypes/series_points.md) instead.

For other plot settings, see [Customize plots](time-series.md#customize-plots).

snippet: howto/visible_series

## Visualize fixed-window plots

The [TimeSeriesView](../../reference/types/views/time_series_view.md) supports direct manipulation of the visible time range.
This allows you to create a plot that only shows a fixed window of data.

Visible time ranges also work in 2D and 3D views, for example for trails.
See [Visible time ranges](../view-concepts/visible-time-ranges.md) for the shared settings, [per-entity ranges](../view-concepts/visible-time-ranges.md#set-the-time-range-per-entity), and [linked time axes](../view-concepts/visible-time-ranges.md#link-the-time-axis-of-several-plots).

Pass a `VisibleTimeRange` to `time_ranges`.
See [How `VisibleTimeRange` works](../view-concepts/visible-time-ranges.md#how-visibletimerange-works) for timelines and boundary types.

### Example syntax
To create a trailing 5 second window plot, you can specify your `TimeSeriesView` like this:
snippet: howto/visualization/blueprint_view_options[fixed_window]

### Complete fixed-window example
For a complete working example, you can run the following code:

snippet: tutorials/fixed_window_plot

This should create a plot that only shows the last 5 seconds of data.
If you select the view, you should see that the time range is configured as expected.

<picture>
  <img src="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/full.png" alt="Time series view of a random walk showing only the last 5 seconds, with the visible time range override in the selection panel">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/fixed_window_example/f76228dc2e1212c148064c2193cdf75ef14bb2b9/1200w.png">
</picture>

Alternatively, you can check out a more full-featured example with multiple plot windows [here](https://github.com/rerun-io/rerun/tree/latest/examples/python/live_scrolling_plot).

### Additional notes
- Any time you log data, it is automatically timestamped on the `log_time` timeline (and on `log_tick`, if enabled).

## Customize plots

For [histograms](bar-chart.md#histograms) and [spectra](bar-chart.md#spectra-and-other-vectors-that-change-over-time), use a bar chart view.
For [XY plots](2d.md#xy-plots-y-against-something-other-than-time), use a 2D view.

### Series names, colors, and line widths

Log [`SeriesLines`](../../reference/types/archetypes/series_lines.md) (or [`SeriesPoints`](../../reference/types/archetypes/series_points.md) for markers) to the same entity as your [`Scalars`](../../reference/types/archetypes/scalars.md).
Log the style as static, since it does not change over time:

snippet: archetypes/series_lines_style

If you log several scalars in one batch, pass one name, color, and width per series.
To hide some of the series, see [Show a subset of plotted series](time-series.md#show-a-subset-of-plotted-series).

You can also set these values for a whole view with blueprint defaults, or per entity with overrides.
See [Customize views](../view-concepts/customize-views.md).

### Axis range and legend

Use `ScalarAxis` for the Y axis, and `PlotLegend` for the legend:

snippet: howto/visualization/blueprint_view_options[axis_and_legend]

The Y axis is always linear.
There is no logarithmic axis yet: plot `log10` of your values instead.
See [missing features](../../development/roadmap.md#missing-features).

To set the X (time) axis window, see [Scrolling plots and the time axis](../view-concepts/visible-time-ranges.md#scrolling-plots-and-the-time-axis).
To pan and zoom several plots together, see [Link the time axis of several plots](../view-concepts/visible-time-ranges.md#link-the-time-axis-of-several-plots).

### Error bands and uncertainty

[`Measurements`](../../reference/types/archetypes/measurements.md) logs values together with their variances.
The time series view draws each series with a band of one standard deviation around it.
`Measurements` is unstable: its data format may change in future releases.

snippet: archetypes/measurements_simple

For spatial uncertainty, see [2D covariance ellipses](2d.md#covariance-ellipses-in-2d) and [3D covariance ellipsoids](3d.md#covariance-ellipsoids-in-3d).

### Plot data that was not logged as `Scalars`

You can plot any numeric component, for example from MCAP files or from `AnyValues`.
See [Plot any scalar](time-series.md#plot-any-scalar).

## Scrolling plots and the time axis

For the difference between queried data and the visible axis window, see [Time ranges and axis windows](../view-concepts/visible-time-ranges.md#scrolling-plots-and-the-time-axis).
For a complete plot recipe, see [Visualize fixed-window plots](#visualize-fixed-window-plots).

## Select visualizers

You can draw scalar data as lines, points, or both.
See [Select visualizers](../view-concepts/customize-views.md#select-visualizers) for the blueprint example.
