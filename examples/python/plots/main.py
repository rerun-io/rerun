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
[rr.Scalar archetype](https://www.rerun.io/docs/reference/types/archetypes/scalar?speculative-link)
archetype.
Each plot is created by logging scalars at different time steps (i.e., the x-axis).
Additionally, the plots are styled using the
[rr.SeriesLine](https://www.rerun.io/docs/reference/types/archetypes/series_line?speculative-link) and
[rr.SeriesPoint](https://www.rerun.io/docs/reference/types/archetypes/series_point?speculative-link)
archetypes respectively.

For the [parabola](recording://curves/parabola) the radius and color is changed over time,
the other plots use timeless for their styling properties where possible.

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
            rr.Scalar(
                f_of_t,
                text="f(t) = (0.01t - 3)³ + 1",
            ),
            rr.SeriesLine(width=width, color=color),
        )


def log_trig() -> None:
    # Styling doesn't change over time, log it once with timeless=True.
    rr.log("trig/sin", rr.SeriesLine(color=[255, 0, 0]), timeless=True)
    rr.log("trig/cos", rr.SeriesLine(color=[0, 255, 0]), timeless=True)

    for t in range(0, int(tau * 2 * 1000.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 1000.0)
        rr.log("trig/sin", rr.Scalar(sin_of_t, text="sin(0.01t)"))

        cos_of_t = cos(float(t) / 1000.0)
        rr.log("trig/cos", rr.Scalar(cos_of_t, text="cos(0.01t)"))


def log_classification() -> None:
    # Log components that don't change only once:
    rr.log("classification/line", rr.SeriesLine(width=3.0))

    for t in range(0, 1000, 2):
        rr.set_time_sequence("frame_nr", t)

        f_of_t = (2 * 0.01 * t) + 2
        color = [255, 255, 0]
        rr.log("classification/line", rr.Scalar(f_of_t), rr.SeriesLine(color=color, width=3.0))

        g_of_t = f_of_t + random.uniform(-5.0, 5.0)
        if g_of_t < f_of_t - 1.5:
            color = [255, 0, 0]
        elif g_of_t > f_of_t + 1.5:
            color = [0, 255, 0]
        else:
            color = [255, 255, 255]
        # radius = abs(g_of_t - f_of_t)
        rr.log("classification/samples", rr.Scalar(g_of_t), rr.SeriesPoint(color=color))  # , radius=radius)) # TODO:


def main() -> None:
    parser = argparse.ArgumentParser(
        description="demonstrates how to integrate python's native `logging` with the Rerun SDK"
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_plot")

    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), timeless=True)
    log_bar_chart()
    log_parabola()
    log_trig()
    log_classification()

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
