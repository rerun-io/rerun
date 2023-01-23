#!/usr/bin/env python3

"""
This example demonstrates how to log simple plots with the Rerun SDK.

Run:
```sh
./examples/plot/main.py
```
"""


import argparse
import pathlib
import random
from math import cos, pi, sin, sqrt, tau

import numpy as np

import rerun as rr


def clamp(n, smallest, largest):  # type: ignore[no-untyped-def]
    return max(smallest, min(n, largest))


def log_bar_chart() -> None:
    # Log a gauss bell as a bar chart
    mean = 0
    std = 1
    variance = np.square(std)
    x = np.arange(-5, 5, 0.01)
    y = np.exp(-np.square(x - mean) / 2 * variance) / (np.sqrt(2 * np.pi * variance))
    rr.log_tensor("bar_chart", y)


def log_parabola() -> None:
    # Log a parabola as a time series
    for t in range(0, 1000, 10):
        rr.set_time_sequence("frame_nr", t)

        f_of_t = (t * 0.01 - 5) ** 3 + 1
        radius = clamp(abs(f_of_t) * 0.1, 0.5, 10.0)  # type: ignore[no-untyped-call]
        color = [255, 255, 0]
        if f_of_t < -10.0:
            color = [255, 0, 0]
        elif f_of_t > 10.0:
            color = [0, 255, 0]

        rr.log_scalar("parabola", f_of_t, label="f(t) = (0.01t - 3)³ + 1", radius=radius, color=color)


def log_trig() -> None:
    # Log a time series
    for t in range(0, int(tau * 2 * 100.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 100.0)
        rr.log_scalar("trig/sin", sin_of_t, label="sin(0.01t)", color=[255, 0, 0])

        cos_of_t = cos(float(t) / 100.0)
        rr.log_scalar("trig/cos", cos_of_t, label="cos(0.01t)", color=[0, 255, 0])


def log_segmentation() -> None:
    # Log a time series
    for t in range(0, 1000, 2):
        rr.set_time_sequence("frame_nr", t)

        f_of_t = (2 * 0.01 * t) + 2
        color = [255, 255, 0]
        rr.log_scalar("segmentation/line", f_of_t, color=color, radius=3.0)

        g_of_t = f_of_t + random.uniform(-5.0, 5.0)
        if g_of_t < f_of_t - 1.5:
            color = [255, 0, 0]
        elif g_of_t > f_of_t + 1.5:
            color = [0, 255, 0]
        else:
            color = [255, 255, 255]
        radius = abs(g_of_t - f_of_t)
        rr.log_scalar("segmentation/samples", g_of_t, color=color, scattered=True, radius=radius)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="demonstrates how to integrate python's native `logging` with the Rerun SDK"
    )
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument(
        "--serve",
        dest="serve",
        action="store_true",
        help="Serve a web viewer (WARNING: experimental feature)",
    )
    args = parser.parse_args()

    rr.init("plot", application_path=pathlib.Path(__file__).resolve())

    if args.serve:
        rr.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rr.connect(args.addr)
    elif args.save is None and not args.headless:
        rr.spawn_and_connect()

    log_parabola()
    log_trig()
    log_segmentation()
    log_bar_chart()

    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            from time import sleep

            sleep(100_000)
        except:
            pass
    elif args.save is not None:
        rr.save(args.save)


if __name__ == "__main__":
    main()
