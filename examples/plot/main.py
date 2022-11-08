#!/usr/bin/env python3

"""
This example demonstrates how to log simple plots with the Rerun SDK.

Run:
```sh
./examples/plot/main.py
```
"""


import argparse

from math import pi

import rerun_sdk as rerun


def log_plots() -> None:
    for i in range(0, int(pi * 3 * 1000.0), 1):
        rerun.set_time_sequence("frame_nr", i)
        rerun.log_scalar("plots/sin", i / 1000.0)



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

    log_plots()

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
