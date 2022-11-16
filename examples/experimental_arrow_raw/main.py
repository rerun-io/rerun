#!/usr/bin/env python3
"""WARNING: Highly experimental and not ready for general use

Directly use logging APIs for raw
"""

import argparse
import logging
import os
from pathlib import Path
from time import sleep
from typing import Any, Final

import rerun
from rerun import rerun_sdk  # type: ignore[attr-defined]


def run_log() -> None:
    rerun_sdk.log_arrow_msg("world/test")
    pass


def main() -> None:
    demos = {
        "log": run_log,
    }

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument(
        "--demo", type=str, default="all", help="What demo to run", choices=["all"] + list(demos.keys())
    )

    parser.add_argument(
        "--connect",
        dest="connect",
        action="store_true",
        help="Connect to an external viewer",
    )
    parser.add_argument(
        "--serve",
        dest="serve",
        action="store_true",
        help="Serve a web viewer (WARNING: experimental feature)",
    )
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")

    args = parser.parse_args()

    rerun.init("arrow")

    if args.serve:
        rerun.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    if args.demo == "all":
        print("Running all demos…")
        for name, demo in demos.items():
            demo()
    else:
        demo = demos[args.demo]
        demo()

    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            sleep(100_000)
        except:
            pass

    elif args.save is not None:
        rerun.save(args.save)
    elif not args.connect:
        # Show the logged data inside the Python process:
        rerun.show()


if __name__ == "__main__":
    main()
