#!/usr/bin/env python3
"""
Demonstrates how to log simple plots with the Rerun SDK.

Run:
```sh
./examples/python/plot/plots.py
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

The full source code for this example is available [on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/plots).
""".strip()


def log_bar_chart() -> None:
    rr.set_time("frame_nr", sequence=0)
    # Log a gauss bell as a bar chart
    mean = 0
    std = 1
    variance = np.square(std)
    x = np.arange(-5, 5, 0.1)
    y = np.exp(-np.square(x - mean) / 2 * variance) / (np.sqrt(2 * np.pi * variance))
    rr.log("bar_chart", rr.BarChart(y))


def log_parabola() -> None:
    # Time-independent styling can be achieved by logging static components to the data store. Here, by using the
    # `SeriesLines` archetype, we further hint the viewer to use the line plot visualizer.
    # Alternatively, you can achieve time-independent styling using overrides, as is everywhere else in this example
    # (see the `main()` function).
    rr.log("curves/parabola", rr.SeriesLines(names="f(t) = (0.01t - 3)³ + 1"), static=True)

    # Log a parabola as a time series
    for t in range(0, 1000, 10):
        rr.set_time("frame_nr", sequence=t)

        f_of_t = (t * 0.01 - 5) ** 3 + 1
        width = np.clip(abs(f_of_t) * 0.1, 0.5, 10.0)
        color = [255, 255, 0]
        if f_of_t < -10.0:
            color = [255, 0, 0]
        elif f_of_t > 10.0:
            color = [0, 255, 0]

        # Note: by using the `rr.SeriesLines` archetype, we hint the viewer to use the line plot visualizer.
        rr.log(
            "curves/parabola",
            rr.Scalars(f_of_t),
            rr.SeriesLines(widths=width, colors=color),
        )


def log_trig() -> None:
    for t in range(int(tau * 2 * 100.0)):
        rr.set_time("frame_nr", sequence=t)

        sin_of_t = sin(float(t) / 100.0)
        rr.log("trig/sin", rr.Scalars(sin_of_t))

        cos_of_t = cos(float(t) / 100.0)
        rr.log("trig/cos", rr.Scalars(cos_of_t))


def log_spiral() -> None:
    times = np.arange(int(tau * 2 * 100.0))
    theta = times / 100.0

    x = theta * np.cos(theta)
    y = theta * np.sin(theta)

    # want this in column major, and numpy is row-major by default
    scalars = np.array((x, y)).T
    rr.send_columns(
        "spiral",
        indexes=[rr.TimeColumn("frame_nr", sequence=times)],
        columns=[*rr.Scalars.columns(scalars=scalars)],
    )


def log_classification() -> None:
    for t in range(0, 1000, 2):
        rr.set_time("frame_nr", sequence=t)

        f_of_t = (2 * 0.01 * t) + 2
        rr.log("classification/line", rr.Scalars(f_of_t))

        g_of_t = f_of_t + random.uniform(-5.0, 5.0)
        if g_of_t < f_of_t - 1.5:
            color = [255, 0, 0]
        elif g_of_t > f_of_t + 1.5:
            color = [0, 255, 0]
        else:
            color = [255, 255, 255]
        marker_size = abs(g_of_t - f_of_t)

        # Note: this log call doesn't include any hint as to which visualizer to use. We use a blueprint visualizer
        # override instead (see `main()`)
        rr.log(
            "classification/samples",
            rr.Scalars(g_of_t),
            rr.SeriesPoints(colors=color, marker_sizes=marker_size),
        )


def main() -> None:
    parser = argparse.ArgumentParser(
        description="demonstrates how to integrate python's native `logging` with the Rerun SDK",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    blueprint = rrb.Blueprint(
        rrb.Horizontal(
            rrb.Vertical(
                rrb.Grid(
                    rrb.BarChartView(name="Bar Chart", origin="/bar_chart"),
                    rrb.TimeSeriesView(
                        name="Curves",
                        origin="/curves",
                    ),
                    rrb.TimeSeriesView(
                        name="Trig",
                        origin="/trig",
                        overrides={
                            "/trig/sin": rr.SeriesLines.from_fields(colors=[255, 0, 0], names="sin(0.01t)"),
                            "/trig/cos": rr.SeriesLines.from_fields(colors=[0, 255, 0], names="cos(0.01t)"),
                        },
                    ),
                    rrb.TimeSeriesView(
                        name="Classification",
                        origin="/classification",
                        overrides={
                            "classification/line": rr.SeriesLines.from_fields(colors=[255, 255, 0], widths=3.0),
                            # This ensures that the `SeriesPoints` visualizers is used for this entity.
                            "classification/samples": rr.SeriesPoints(),
                        },
                    ),
                ),
                rrb.TimeSeriesView(
                    name="Spiral",
                    origin="/spiral",
                    overrides={"spiral": rr.SeriesLines.from_fields(names=["0.01t cos(0.01t)", "0.01t sin(0.01t)"])},  # type: ignore[arg-type]
                ),
                row_shares=[2, 1],
            ),
            rrb.TextDocumentView(name="Description", origin="/description"),
            column_shares=[3, 1],
        ),
        rrb.SelectionPanel(state="collapsed"),
        rrb.TimePanel(state="collapsed"),
    )

    rr.script_setup(args, "rerun_example_plot", default_blueprint=blueprint)

    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)
    log_bar_chart()
    log_parabola()
    log_trig()
    log_spiral()
    log_classification()

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
