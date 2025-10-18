#!/usr/bin/env python3
"""
Plot dashboard stress test.

Usage:
-----
```
pixi run py-plot-dashboard --help
```

Example:
-------
```
pixi run py-plot-dashboard --num-plots 10 --num-series-per-plot 5 --num-points-per-series 5000 --freq 1000
```

"""

from __future__ import annotations

import argparse
import math
import time
from typing import Any, cast

import numpy as np
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb

parser = argparse.ArgumentParser(description="Plot dashboard stress test")
rr.script_add_args(parser)

parser.add_argument("--num-plots", type=int, default=1, help="How many different plots?")
parser.add_argument(
    "--num-series-per-plot",
    type=int,
    default=1,
    help="How many series in each single plot?",
)
parser.add_argument(
    "--num-points-per-series",
    type=int,
    default=100000,
    help="How many points in each single series?",
)
parser.add_argument(
    "--freq",
    type=float,
    default=1000,
    help="Frequency of logging (applies to all series)",
)
parser.add_argument(
    "--temporal-batch-size",
    type=int,
    default=None,
    help="Number of rows to include in each log call",
)
parser.add_argument(
    "--blueprint",
    action="store_true",
    help="Setup a blueprint for a 5s window",
)

order = [
    "forwards",
    "backwards",
    "random",
]
parser.add_argument(
    "--order",
    type=str,
    default=order[0],
    help="What order to log the data in (applies to all series)",
    choices=order,
)

series_type = [
    "gaussian-random-walk",
    "sin-uniform",
]
parser.add_argument(
    "--series-type",
    type=str,
    default=series_type[0],
    choices=series_type,
    help="The method used to generate time series",
)


# TODO(cmc): could have flags to add attributes (color, radius...) to put some more stress
# on the line fragmenter.

args = parser.parse_args()


def main() -> None:
    rr.script_setup(args, "rerun_example_plot_dashboard_stress")

    plot_paths = [f"plot_{i}" for i in range(args.num_plots)]
    series_paths = [f"series_{i}" for i in range(args.num_series_per_plot)]

    if args.blueprint:
        print("logging blueprint!")
        rr.send_blueprint(
            rrb.Blueprint(
                rrb.Grid(*[
                    rrb.TimeSeriesView(
                        name=p,
                        origin=f"/{p}",
                        time_ranges=rrb.VisibleTimeRanges(
                            timeline="sim_time",
                            start=rrb.TimeRangeBoundary.cursor_relative(offset=rr.TimeInt(seconds=-2.5)),
                            end=rrb.TimeRangeBoundary.cursor_relative(offset=rr.TimeInt(seconds=2.5)),
                        ),
                    )
                    for p in plot_paths
                ]),
                rrb.BlueprintPanel(state="collapsed"),
                rrb.SelectionPanel(state="collapsed"),
            ),
        )

    time_per_sim_step = 1.0 / args.freq
    stop_time = args.num_points_per_series * time_per_sim_step

    if args.order == "forwards":
        sim_times = np.arange(0, stop_time, time_per_sim_step)
    elif args.order == "backwards":
        sim_times = np.arange(0, stop_time, time_per_sim_step)[::-1]
    else:
        sim_times = np.random.randint(0, args.num_points_per_series)

    num_series = len(plot_paths) * len(series_paths)
    time_per_tick = time_per_sim_step
    scalars_per_tick = num_series
    if args.temporal_batch_size is not None:
        time_per_tick *= args.temporal_batch_size
        scalars_per_tick *= args.temporal_batch_size

    expected_total_freq = args.freq * num_series

    values_shape = (
        len(sim_times),
        len(plot_paths),
        len(series_paths),
    )
    if args.series_type == "gaussian-random-walk":
        values = np.cumsum(np.random.normal(size=values_shape), axis=0)
    elif args.series_type == "sin-uniform":
        values = np.sin(np.random.uniform(0, math.pi, size=values_shape))
    else:
        # Just generate random numbers rather than crash
        values = np.random.normal(size=values_shape)

    if args.temporal_batch_size is None:
        ticks: Any = enumerate(sim_times)
    else:
        offsets = range(0, len(sim_times), args.temporal_batch_size)
        ticks = zip(
            offsets,
            (sim_times[offset : offset + args.temporal_batch_size] for offset in offsets),
            strict=False,
        )

    time_column = None

    total_start_time = time.time()
    total_num_scalars = 0

    tick_start_time = time.time()
    max_load = 0.0

    for index, sim_time in ticks:
        if args.temporal_batch_size is None:
            rr.set_time("sim_time", duration=sim_time)
        else:
            time_column = rr.TimeColumn("sim_time", duration=sim_time)

        # Log
        for plot_idx, plot_path in enumerate(plot_paths):
            for series_idx, series_path in enumerate(series_paths):
                if args.temporal_batch_size is None:
                    value = values[index, plot_idx, series_idx]
                    rr.log(f"{plot_path}/{series_path}", rr.Scalars(value))
                else:
                    value_index = slice(index, index + args.temporal_batch_size)
                    rr.send_columns(
                        f"{plot_path}/{series_path}",
                        indexes=[cast("rr.TimeColumn", time_column)],
                        columns=rr.Scalars.columns(scalars=values[value_index, plot_idx, series_idx]),
                    )

        # Measure how long this took and how high the load was.

        elapsed = time.time() - tick_start_time
        max_load = max(max_load, elapsed / time_per_tick)

        # Throttle

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

        # Progress report
        #
        # Must come after throttle since we report every wall-clock second:
        # If ticks are large & fast, then after each send we run into throttle.
        # So if this was before throttle, we'd not report the first tick no matter how large it was.

        total_num_scalars += scalars_per_tick
        total_elapsed = time.time() - total_start_time

        if total_elapsed >= 1.0:
            print(
                f"logged {total_num_scalars} scalars over {round(total_elapsed, 3)}s \
(freq={round(total_num_scalars / total_elapsed, 3)}Hz, expected={round(expected_total_freq, 3)}Hz, \
load={round(max_load * 100.0, 3)}%)",
            )

            elapsed_debt = total_elapsed % 1  # just keep the fractional part
            total_start_time = time.time() - elapsed_debt
            total_num_scalars = 0
            max_load = 0.0

    if total_num_scalars > 0:
        total_elapsed = time.time() - total_start_time
        print(
            f"logged {total_num_scalars} scalars over {round(total_elapsed, 3)}s \
(freq={round(total_num_scalars / total_elapsed, 3)}Hz, expected={round(expected_total_freq, 3)}Hz, \
load={round(max_load * 100.0, 3)}%)",
        )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
