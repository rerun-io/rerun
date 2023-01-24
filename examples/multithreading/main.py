#!/usr/bin/env python3

"""
Demonstration of using rerun from multiple threads
"""

import argparse
import random
import threading
import time

import numpy as np
import numpy.typing as npt
from rerun.log.rects import RectFormat

import rerun as rr


def rect_logger(path: str, color: npt.NDArray[np.float32]) -> None:
    for _ in range(1000):
        rects_xy = np.random.rand(5, 2) * 1024
        rects_wh = np.random.rand(5, 2) * (1024 - rects_xy + 1)
        rects = np.hstack((rects_xy, rects_wh))
        rr.log_rects(path, rects, colors=color, rect_format=RectFormat.XYWH)


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
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

    rr.init("multithreading")

    if args.serve:
        rr.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rr.connect(args.addr)
    elif args.save is None and not args.headless:
        rr.spawn_and_connect()

    for i in range(10):
        t = threading.Thread(
            target=rect_logger, args=("thread/{}".format(i), [random.randrange(255) for _ in range(3)])
        )
        t.start()

    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            time.sleep(100_000)
        except:
            pass
    elif args.save is not None:
        rr.save(args.save)


if __name__ == "__main__":
    main()
