---
title: Bar chart
order: 50
description: Compare numeric values as bars
---

Use a bar chart view to compare a set of numeric values at the current time.
It is useful for comparing class scores, counts, or histogram bins when the relative size of each value matters more than its history.

See the [`BarChartView` reference](../../reference/types/views/bar_chart_view.md) for properties and examples.

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
