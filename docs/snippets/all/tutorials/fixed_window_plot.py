#!/usr/bin/env python3
"""A live plot of a random walk using a scrolling fixed window size."""

from __future__ import annotations

import time
from typing import Iterator

import numpy as np
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb


def random_walk_generator() -> Iterator[float]:
    value = 0.0
    while True:
        value += np.random.normal()
        yield value


rr.init("rerun_example_fixed_window_plot", spawn=True)

rr.send_blueprint(
    rrb.TimeSeriesView(
        origin="random_walk",
        time_ranges=[
            rrb.VisibleTimeRange(
                "time",
                start=rrb.TimeRangeBoundary.cursor_relative(seconds=-5.0),
                end=rrb.TimeRangeBoundary.cursor_relative(),
            )
        ],
    )
)

values = random_walk_generator()

cur_time = time.time()

while True:
    cur_time += 0.01
    sleep_for = cur_time - time.time()
    if sleep_for > 0:
        time.sleep(sleep_for)

    rr.set_time_seconds("time", cur_time)

    rr.log("random_walk", rr.Scalar(next(values)))
