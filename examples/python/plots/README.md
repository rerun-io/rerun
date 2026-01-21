<!--[metadata]
title = "Plots"
tags = ["2D", "Plots", "API example"]
thumbnail = "https://static.rerun.io/plots/e8e51071f6409f61dc04a655d6b9e1caf8179226/480w.png"
thumbnail_dimensions = [480, 480]
channel = "main"
include_in_manifest = true
-->

This example demonstrates how to log simple plots with the Rerun SDK. Charts can be created from 1-dimensional tensors, or from time-varying scalars.

<picture data-inline-viewer="examples/plots">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/1200w.png">
  <img src="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/full.png" alt="Plots example screenshot">
</picture>

## Used Rerun types

[`BarChart`](https://www.rerun.io/docs/reference/types/archetypes/bar_chart), [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars), [`SeriesPoints`](https://www.rerun.io/docs/reference/types/archetypes/series_points), [`SeriesLines`](https://www.rerun.io/docs/reference/types/archetypes/series_lines), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

## Logging and visualizing with Rerun

This example shows various plot types that you can create using Rerun. Common usecases for such plots would be logging
losses or metrics over time, histograms, or general function plots.

The bar chart is created by logging the [`BarChart`](https://www.rerun.io/docs/reference/types/archetypes/bar_chart) archetype.
All other plots are created using the [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars) archetype.
Each plot is created by logging scalars at different time steps (i.e., the x-axis).
Additionally, the plots are styled using the [`SeriesLines`](https://www.rerun.io/docs/reference/types/archetypes/series_lines) and
[`SeriesPoints`](https://www.rerun.io/docs/reference/types/archetypes/series_points) archetypes respectively.

The visualizations in this example were created with the following Rerun code:

### Bar chart

The `log_bar_chart` function logs a bar chat.
It generates data for a Gaussian bell curve and logs it using [`BarChart`](https://www.rerun.io/docs/reference/types/archetypes/bar_chart) archetype.
```python
def log_bar_chart() -> None:
    # … existing code …
    rr.log("bar_chart", rr.BarChart(y))
```

### Curves
The `log_parabola` function logs a parabola curve (sine and cosine functions) as a time series.

It first sets up a time sequence using [`timelines`](https://www.rerun.io/docs/concepts/timelines), then calculates the y-value of the parabola at each time step, and logs it using [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars) archetype.
It also adjusts the width and color of the plotted line based on the calculated y value using [`SeriesLines`](https://www.rerun.io/docs/reference/types/archetypes/series_lines) archetype.

```python
def log_parabola() -> None:
    # Name never changes, log it only once.
    rr.log("curves/parabola", rr.SeriesLines(name="f(t) = (0.01t - 3)³ + 1"), static=True)

    # Log a parabola as a time series
    for t in range(0, 1000, 10):
        rr.set_time("frame_nr", sequence=t)

        # … existing code …

        rr.log(
            "curves/parabola",
            rr.Scalars(f_of_t),
            rr.SeriesLines(width=width, color=color),
        )
```

### Trig

The `log_trig` function logs sin and cos functions as time series. Sin and cos are logged with the same parent entity (i.e.,`trig/{cos,sin}`) which will put them in the same view by default.

It first logs the styling properties of the sin and cos plots using [`SeriesLines`](https://www.rerun.io/docs/reference/types/archetypes/series_lines) archetype.
Then, it iterates over a range of time steps, calculates the sin and cos values at each time step, and logs them using [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars) archetype.

 ```python
def log_trig() -> None:
    # Styling doesn't change over time, log it once with static=True.
    rr.log("trig/sin", rr.SeriesLines(color=[255, 0, 0], name="sin(0.01t)"), static=True)
    rr.log("trig/cos", rr.SeriesLines(color=[0, 255, 0], name="cos(0.01t)"), static=True)

    for t in range(0, int(tau * 2 * 100.0)):
        rr.set_time("frame_nr", sequence=t)

        sin_of_t = sin(float(t) / 100.0)
        rr.log("trig/sin", rr.Scalars(sin_of_t))

        cos_of_t = cos(float(t) / 100.0)
        rr.log("trig/cos", rr.Scalars(cos_of_t))
 ```

### Classification

The `log_classification` function simulates a classification problem by logging a line function and randomly generated samples around that line.

It first logs the styling properties of the line plot using [`SeriesLines`](https://www.rerun.io/docs/reference/types/archetypes/series_lines) archetype.
Then, it iterates over a range of time steps, calculates the y value of the line function at each time step, and logs it as a scalars using [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars) archetype.
Additionally, it generates random samples around the line function and logs them using [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars) and [`SeriesPoints`](https://www.rerun.io/docs/reference/types/archetypes/series_points) archetypes.

 ```python
def log_classification() -> None:
    # Log components that don't change only once:
    rr.log("classification/line", rr.SeriesLines(colors=[255, 255, 0], widths=3.0), static=True)

    for t in range(0, 1000, 2):
        rr.set_time("frame_nr", sequence=t)

        # … existing code …
        rr.log("classification/line", rr.Scalars(f_of_t))

        # … existing code …
        rr.log("classification/samples", rr.Scalars(g_of_t), rr.SeriesPoints(colors=color, marker_sizes=marker_size))
 ```


## Run the code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/plots
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m plots # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m plots --help
```

## Advanced time series - [`send_columns`](https://ref.rerun.io/docs/python/stable/common/columnar_api/#rerun.send_columns)
Logging many scalars individually can be slow.
The [`send_columns`](https://ref.rerun.io/docs/python/stable/common/columnar_api/#rerun.send_columns) API can be used to log many scalars at once.
Check the [`Scalars` `send_columns` snippet](https://rerun.io/docs/reference/types/archetypes/scalars#update-a-scalar-over-time-in-a-single-operation) to learn more.
