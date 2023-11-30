#!/usr/bin/env python3
"""Query caching stress test."""
from __future__ import annotations

import argparse
import math
import random
import time

import numpy as np
import rerun as rr  # pip install rerun-sdk

parser = argparse.ArgumentParser(description="Query caching stress test")
rr.script_add_args(parser)

# TODO: attribute shenanigans
parser.add_argument("--num-plots", type=int, default=1, help="How many different plots")
parser.add_argument("--num-series-per-plot", type=int, default=1, help="How many series in each single plot")
parser.add_argument("--num-points-per-plot", type=int, default=10000000, help="How many points in each single plot")
parser.add_argument("--freq-per-plot", type=float, help="Frequency of logging for each single plot (as fast as possible by default)")

order = [ "forwards", "backwards", "random", ]
parser.add_argument("--order", type=str, default="forwards", help="What order to log the data in (applies to all plots)")

args = parser.parse_args()

def main() -> None:
    rr.script_setup(args, "rerun_example_query_caching_stress_test")

    num_points = args.num_points_per_plot
    sleep_time = 1.0 / args.freq_per_plot

    plot_paths = [f"plot_{i}" for i in range(0, args.num_plots)]
    series_paths = [f"series_{i}" for i in range(0, args.num_series_per_plot)]

    if args.order == "forwards":
        times = np.arange(num_points)
    elif args.order == "backwards":
        times = np.arange(num_points)[::-1]
    else:
        times = np.random.randint(0, num_points)

    for t in times:
        now = time.time()
        rr.set_time_sequence("sim_tick", t)
        rr.set_time_seconds("sim_time", t)

        for plot_path in plot_paths:
            for series_path in series_paths:
                value = math.sin(random.uniform(0.0, math.pi))
                rr.log(f"{plot_path}/{series_path}", rr.TimeSeriesScalar(value))

        elapsed = time.time() - now
        if args.freq_per_plot is not None:
            secs = sleep_time - elapsed
            if secs < 0.0:
                print(f"WARN: script is too slow to support a frequency of {args.freq_per_plot}Hz")
            else:
                time.sleep(secs)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
