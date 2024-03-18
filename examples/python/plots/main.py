#!/usr/bin/env python3
"""
Demonstrates how to log simple plots with the Rerun SDK.

Run:
```sh
./examples/python/plot/main.py
```
"""
from __future__ import annotations

import argparse
import random
from math import cos, sin, tau

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

DESCRIPTION = """
# Plots
This example shows various plot types that you can create using Rerun. Common usecases for such plots would be logging
losses or metrics over time, histograms, or general function plots.

## How it was made
The full source code for this example is available [on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/plots/main.py).

### Bar charts
The [bar chart](recording://bar_chart) is created by logging the [rr.BarChart archetype](https://www.rerun.io/docs/reference/types/archetypes/bar_chart).

### Time series
All other plots are created using the
[rr.Scalar archetype](https://www.rerun.io/docs/reference/types/archetypes/scalar)
archetype.
Each plot is created by logging scalars at different time steps (i.e., the x-axis).
Additionally, the plots are styled using the
[rr.SeriesLine](https://www.rerun.io/docs/reference/types/archetypes/series_line) and
[rr.SeriesPoint](https://www.rerun.io/docs/reference/types/archetypes/series_point)
archetypes respectively.

For the [parabola](recording://curves/parabola) the radius and color is changed over time,
the other plots use static for their styling properties where possible.

[sin](recording://trig/sin) and [cos](recording://trig/cos) are logged with the same parent entity (i.e.,
`trig/{cos,sin}`) which will put them in the same view by default.
""".strip()


def clamp(n, smallest, largest):  # type: ignore[no-untyped-def]
    return max(smallest, min(n, largest))


def log_bar_chart() -> None:
    rr.set_time_sequence("frame_nr", 0)
    # Log a gauss bell as a bar chart
    mean = 0
    std = 1
    variance = np.square(std)
    x = np.arange(-5, 5, 0.1)
    y = np.exp(-np.square(x - mean) / 2 * variance) / (np.sqrt(2 * np.pi * variance))
    rr.log("bar_chart", rr.BarChart(y))


def log_parabola() -> None:
    # Name never changes, log it only once.
    rr.log("curves/parabola", rr.SeriesLine(name="f(t) = (0.01t - 3)³ + 1"), static=True)

    # Log a parabola as a time series
    for t in range(0, 1000, 10):
        rr.set_time_sequence("frame_nr", t)

        f_of_t = (t * 0.01 - 5) ** 3 + 1
        width = clamp(abs(f_of_t) * 0.1, 0.5, 10.0)
        color = [255, 255, 0]
        if f_of_t < -10.0:
            color = [255, 0, 0]
        elif f_of_t > 10.0:
            color = [0, 255, 0]

        rr.log(
            "curves/parabola",
            rr.Scalar(f_of_t),
            rr.SeriesLine(width=width, color=color),
        )


def log_trig() -> None:
    # Styling doesn't change over time, log it once with static=True.
    rr.log("trig/sin", rr.SeriesLine(color=[255, 0, 0], name="sin(0.01t)"), static=True)
    rr.log("trig/cos", rr.SeriesLine(color=[0, 255, 0], name="cos(0.01t)"), static=True)

    for t in range(0, int(tau * 2 * 100.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 100.0)
        rr.log("trig/sin", rr.Scalar(sin_of_t))

        cos_of_t = cos(float(t) / 100.0)
        rr.log("trig/cos", rr.Scalar(cos_of_t))


def log_classification() -> None:
    # Log components that don't change only once:
    rr.log("classification/line", rr.SeriesLine(color=[255, 255, 0], width=3.0), static=True)

    for t in range(0, 1000, 2):
        rr.set_time_sequence("frame_nr", t)

        f_of_t = (2 * 0.01 * t) + 2
        rr.log("classification/line", rr.Scalar(f_of_t))

        g_of_t = f_of_t + random.uniform(-5.0, 5.0)
        if g_of_t < f_of_t - 1.5:
            color = [255, 0, 0]
        elif g_of_t > f_of_t + 1.5:
            color = [0, 255, 0]
        else:
            color = [255, 255, 255]
        marker_size = abs(g_of_t - f_of_t)
        rr.log("classification/samples", rr.Scalar(g_of_t), rr.SeriesPoint(color=color, marker_size=marker_size))


def main() -> None:
    parser = argparse.ArgumentParser(
        description="demonstrates how to integrate python's native `logging` with the Rerun SDK"
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    blueprint = rrb.Blueprint(
        rrb.Horizontal(
            rrb.Grid(
                rrb.BarChartView(name="Bar Chart", origin="/bar_chart"),
                rrb.TimeSeriesView(name="Curves", origin="/curves"),
                rrb.TimeSeriesView(name="Trig", origin="/trig"),
                rrb.TimeSeriesView(name="Classification", origin="/classification"),
            ),
            rrb.TextDocumentView(name="Description", origin="/description"),
            column_shares=[2, 1],
        ),
        rrb.SelectionPanel(expanded=False),
        rrb.TimePanel(expanded=False),
    )

    rr.script_setup(args, "rerun_example_plot", blueprint=blueprint)

    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)
    log_bar_chart()
    log_parabola()
    log_trig()
    log_classification()

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
