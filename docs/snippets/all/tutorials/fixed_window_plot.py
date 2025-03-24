#!/usr/bin/env python3
"""A live plot of a random walk using a scrolling fixed window size."""

from __future__ import annotations

import time

import numpy as np
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb

rr.init("rerun_example_fixed_window_plot", spawn=True)

rr.send_blueprint(
    rrb.TimeSeriesView(
        origin="random_walk",
        time_ranges=rrb.VisibleTimeRange(
            "time",
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-5.0),
            end=rrb.TimeRangeBoundary.cursor_relative(),
        ),
    ),
)

cur_time = time.time()
value = 0.0

while True:
    cur_time += 0.01
    sleep_for = cur_time - time.time()
    if sleep_for > 0:
        time.sleep(sleep_for)

    value += np.random.normal()

    rr.set_time("time", timestamp=cur_time)

    rr.log("random_walk", rr.Scalar(value))
