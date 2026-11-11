---
title: Customize plots
order: 150
description: Set plot series names, colors, and line widths, axis ranges, error bands, XY plots, histograms, and spectra that change over time
---

This page collects common questions about plots in the [`TimeSeriesView`](../../reference/types/views/time_series_view.md) and the [`BarChartView`](../../reference/types/views/bar_chart_view.md).

## Series names, colors, and line widths

Log [`SeriesLines`](../../reference/types/archetypes/series_lines.md) (or [`SeriesPoints`](../../reference/types/archetypes/series_points.md) for markers) to the same entity as your [`Scalars`](../../reference/types/archetypes/scalars.md).
Log the style as static, since it does not change over time:

snippet: archetypes/series_lines_style

If you log several scalars in one batch, pass one name, color, and width per series.
To hide some of the series, see [Show a subset of plotted series](visible-series.md).

You can also set these values for a whole view with blueprint defaults, or per entity with overrides.
See [Customize views](../blueprints-and-views/customize-views.md).

## Axis range and legend

Use `ScalarAxis` for the Y axis, and `PlotLegend` for the legend:

```python
rrb.TimeSeriesView(
    origin="/sensors",
    axis_y=rrb.ScalarAxis(range=(-1.0, 1.0), zoom_lock=True),
    plot_legend=rrb.PlotLegend(visible=False),
)
```

The Y axis is always linear.
There is no logarithmic axis yet: plot `log10` of your values instead.
See [missing features](../../development/roadmap.md#missing-features).

To set the X (time) axis window, or to link the time axis of several plots, see [Show trails, history, and sliding time windows](visible-time-ranges.md#scrolling-plots-and-the-time-axis).

## Error bands and uncertainty

[`Measurements`](../../reference/types/archetypes/measurements.md) logs values together with their variances.
The time series view draws each series with a band of one standard deviation around it.
`Measurements` is unstable: its data format may change in future releases.

snippet: archetypes/measurements_simple

See [Visualize uncertainty](../scenes/uncertainty.md) for 2D and 3D uncertainty.

## XY plots (Y against something other than time)

The X axis of a time series view is always a timeline.
To plot Y against another value, for example a trajectory or a phase plot, log the points as [`Points2D`](../../reference/types/archetypes/points2d.md) or [`LineStrips2D`](../../reference/types/archetypes/line_strips2d.md) and show them in a [2D view](../../reference/types/views/spatial2d_view.md):

```python
import numpy as np
import rerun as rr

rr.init("rerun_example_xy_plot", spawn=True)

x = np.linspace(-3.0, 3.0, 200)
y = np.sin(x) * x
rr.log("xy_plot", rr.LineStrips2D([np.column_stack([x, -y])]))
```

The 2D view has its Y axis pointing down, like images, so flip the sign of Y.

## Histograms

Use [`BarChart`](../../reference/types/archetypes/bar_chart.md) with precomputed bins.
`abscissa` sets the left edge of each bar, and `widths` sets the width of each bin, in X axis units:

```python
import numpy as np
import rerun as rr

rr.init("rerun_example_histogram", spawn=True)

samples = np.random.default_rng(0).normal(size=10_000)
counts, edges = np.histogram(samples, bins=50)
rr.log(
    "histogram",
    rr.BarChart(counts, abscissa=edges[:-1], widths=np.diff(edges)),
)
```

`values` can be any numeric type.

## Spectra and other vectors that change over time

To show a vector per time step, such as an FFT spectrum, a forecast horizon, or a lidar scan line, log a new `BarChart` (or a `LineStrips2D` in a 2D view) on every time step.
Then move the time cursor to see the vector at that time.

```python
for step, spectrum in enumerate(spectra):
    rr.set_time("step", sequence=step)
    rr.log("spectrum", rr.BarChart(spectrum, abscissa=frequencies))
```

There is no waterfall or spectrogram view yet.
As a workaround, log the spectrogram as an [`Image`](../../reference/types/archetypes/image.md) (one column per time step), or as a [`Tensor`](../../reference/types/archetypes/tensor.md).
See [missing features](../../development/roadmap.md#missing-features).

## Plot data that was not logged as `Scalars`

You can plot any numeric component, for example from MCAP files or from `AnyValues`.
See [Plot any scalar](plot-any-scalar.md).
