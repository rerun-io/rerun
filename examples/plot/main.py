#!/usr/bin/env python3

"""
This example demonstrates how to log simple plots with the Rerun SDK.

Run:
```sh
./examples/plot/main.py
```
"""


import argparse

from math import tau, pi, sin, cos

import rerun_sdk as rerun

def clamp(n, smallest, largest):
    return max(smallest, min(n, largest))

def log_cubic() -> None:
    for t in range(0, 1000, 10):
        rerun.set_time_sequence("frame_nr", t)

        f_of_t = (t * 0.01 - 3) ** 3 + 1
        radius = clamp(abs(f_of_t), 0.5, 4.0);
        color = [255, 255, 255, 255]
        if f_of_t < 0.0:
            color = [255, 0, 0, 255]
        elif f_of_t > 10.0:
            color = [0, 255, 0, 255]

        rerun.log_scalar("f(t) = (t * 0.01 - 3)³ + 1", f_of_t, radius=radius, color=color)


def log_plots() -> None:
    frame_nr = 1
    for i in range(0, int(tau * 3 * 100.0)):
        rerun.set_time_sequence("frame_nr", frame_nr)
        rerun.log_scalar("plots/sin(t)", sin(i / 100.0))
        rerun.log_scalar("plots/cos(t)", cos(i / 100.0))
        frame_nr += 1


def main():
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

    rerun.init("plot")

    if args.serve:
        rerun.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    log_cubic()
    # log_plots()

    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            from time import sleep

            sleep(100_000)
        except:
            pass
    elif args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()


if __name__ == "__main__":
    main()
