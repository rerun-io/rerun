#!/usr/bin/env python3
"""
Plot dashboard stress test.

Usage:
-----
```
just py-plot-dashboard --help
```

Example:
-------
```
just py-plot-dashboard --num-plots 10 --num-series-per-plot 5 --num-points-per-series 5000 --freq 1000
```

"""
from __future__ import annotations

import argparse
import math
import random
import time

import numpy as np
import rerun as rr  # pip install rerun-sdk

parser = argparse.ArgumentParser(description="Plot dashboard stress test")
rr.script_add_args(parser)

parser.add_argument("--num-plots", type=int, default=1, help="How many different plots?")
parser.add_argument("--num-series-per-plot", type=int, default=1, help="How many series in each single plot?")
parser.add_argument("--num-points-per-series", type=int, default=100000, help="How many points in each single series?")
parser.add_argument("--freq", type=float, default=1000, help="Frequency of logging (applies to all series)")

order = [
    "forwards",
    "backwards",
    "random",
]
parser.add_argument(
    "--order", type=str, default="forwards", help="What order to log the data in (applies to all series)", choices=order
)
parser.add_argument(
    "--series_type",
    type=str,
    default="gaussian_random_walk",
    choices=("gaussian_random_walk", "sin_uniform"),
    help="The method used to generate time series",
)


# TODO(cmc): could have flags to add attributes (color, radius...) to put some more stress
# on the line fragmenter.

args = parser.parse_args()


def main() -> None:
    rr.script_setup(args, "rerun_example_plot_dashboard_stress")

    plot_paths = [f"plot_{i}" for i in range(0, args.num_plots)]
    series_paths = [f"series_{i}" for i in range(0, args.num_series_per_plot)]

    num_series = len(plot_paths) * len(series_paths)
    time_per_tick = 1.0 / args.freq
    expected_total_freq = args.freq * num_series
    stop_time = args.num_points_per_series * time_per_tick

    if args.order == "forwards":
        sim_times = np.arange(0, stop_time, time_per_tick)
    elif args.order == "backwards":
        sim_times = np.arange(0, stop_time, time_per_tick)[::-1]
    else:
        sim_times = np.random.randint(0, args.num_points_per_series)

    total_start_time = time.time()
    total_num_scalars = 0

    tick_start_time = time.time()
    max_load = 0.0

    values_shape = (
        len(sim_times),
        len(plot_paths),
        len(series_paths),
    )
    if args.series_type == "gaussian_random_walk":
        values = np.cumsum(np.random.normal(size=values_shape), axis=0)
    elif args.series_type == "sin_uniform":
        values = np.sin(np.random.uniform(0, math.pi, size=values_shape))
    else:
        # Just generate random numbers rather than crash
        values = np.random.normal(size=values_shape)

    for time_step, sim_time in enumerate(sim_times):
        rr.set_time_seconds("sim_time", sim_time)
        # Log
        for plot_idx, plot_path in enumerate(plot_paths):
            for series_idx, series_path in enumerate(series_paths):
                value = values[time_step, plot_idx, series_idx]
                rr.log(f"{plot_path}/{series_path}", rr.TimeSeriesScalar(value))

        # Progress report

        total_num_scalars += num_series
        total_elapsed = time.time() - total_start_time
        if total_elapsed >= 1.0:
            print(
                f"logged {total_num_scalars} scalars over {round(total_elapsed, 3)}s \
(freq={round(total_num_scalars/total_elapsed, 3)}Hz, expected={round(expected_total_freq, 3)}Hz, \
load={round(max_load * 100.0, 3)}%)"
            )

            elapsed_debt = total_elapsed % 1  # just keep the fractional part
            total_start_time = time.time() - elapsed_debt
            total_num_scalars = 0
            max_load = 0.0

        # Throttle

        elapsed = time.time() - tick_start_time
        sleep_duration = time_per_tick - elapsed
        if sleep_duration > 0.0:
            sleep_start_time = time.time()
            time.sleep(sleep_duration)
            sleep_elapsed = time.time() - sleep_start_time

            # We will very likely be put to sleep for more than we asked for, and therefore need
            # to pay off that debt in order to meet our frequency goal.
            sleep_debt = sleep_elapsed - sleep_duration
            tick_start_time = time.time() - sleep_debt
        else:
            tick_start_time = time.time()

        max_load = max(max_load, elapsed / time_per_tick)

    total_elapsed = time.time() - total_start_time
    print(
        f"logged {total_num_scalars} scalars over {round(total_elapsed, 3)}s \
(freq={round(total_num_scalars/total_elapsed, 3)}Hz, expected={round(expected_total_freq, 3)}Hz, \
load={round(max_load * 100.0, 3)}%)"
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
