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
import rerun as rr  # pip install rerun-sdk

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
[rr.TimeSeriesScalar archetype](https://www.rerun.io/docs/reference/types/archetypes/bar_chart)
with different settings. Each plot is created by logging scalars at different time steps (i.e., the x-axis).

For the [parabola](recording://curves/parabola) the radius and color is changed over time.

[sin](recording://trig/sin) and [cos](recording://trig/cos) are logged with the same parent entity (i.e.,
`trig/{cos,sin}`) which will put them in the same view by default.

For the [classification samples](recording://classification/samples) `rr.TimeSeriesScalar(..., scatter=True)` is used to
create separate points that do not connect over time. Note, that in the other plots the logged scalars are connected
over time by lines.
""".strip()


def clamp(n, smallest, largest):  # type: ignore[no-untyped-def]
    return max(smallest, min(n, largest))


def log_bar_chart() -> None:
    rr.set_time_sequence("frame_nr", 0)
    # Log a gauss bell as a bar chart
    x = np.arange(-50, 50, 0.1)
    rr.log("bar_chart", rr.BarChart(np.sin(x) * 0.5))


def log_parabola() -> None:
    # Log a parabola as a time series
    for t in range(0, 1000000000, 1):
        rr.set_time_sequence("frame_nr", t)

        f_of_t = (t * 0.0001 - 5) ** 3 + 1
        radius = clamp(abs(f_of_t) * 0.1, 0.5, 10.0)
        color = [255, 255, 0]
        if f_of_t < -10.0:
            color = [255, 0, 0]
        elif f_of_t > 1000.0:
            color = [0, 255, 0]

        rr.log(
            "curves/parabola",
            rr.TimeSeriesScalar(
                f_of_t,
                # label="f(t) = (0.01t - 3)³ + 1",
                # radius=radius,
                color=color,
            ),
        )


def log_trig() -> None:
    # Log a time series
    for t in range(0, int(tau * 2 * 100.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 100.0)
        rr.log("trig/sin", rr.TimeSeriesScalar(sin_of_t, label="sin(0.01t)", color=[255, 0, 0]))

        cos_of_t = cos(float(t) / 100.0)
        rr.log("trig/cos", rr.TimeSeriesScalar(cos_of_t, label="cos(0.01t)", color=[0, 255, 0]))


def log_classification() -> None:
    # Log a time series
    for t in range(0, 1000, 2):
        rr.set_time_sequence("frame_nr", t)

        f_of_t = (2 * 0.01 * t) + 2
        color = [255, 255, 0]
        rr.log("classification/line", rr.TimeSeriesScalar(f_of_t, color=color, radius=3.0))

        g_of_t = f_of_t + random.uniform(-5.0, 5.0)
        if g_of_t < f_of_t - 1.5:
            color = [255, 0, 0]
        elif g_of_t > f_of_t + 1.5:
            color = [0, 255, 0]
        else:
            color = [255, 255, 255]
        radius = abs(g_of_t - f_of_t)
        rr.log("classification/samples", rr.TimeSeriesScalar(g_of_t, color=color, scattered=True, radius=radius))


def main() -> None:
    parser = argparse.ArgumentParser(
        description="demonstrates how to integrate python's native `logging` with the Rerun SDK"
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_plot")

    # rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), timeless=True)
    # log_bar_chart()
    log_parabola()
    # log_trig()
    # log_classification()

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
