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
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "multithreading")

    for i in range(10):
        t = threading.Thread(
            target=rect_logger, args=("thread/{}".format(i), [random.randrange(255) for _ in range(3)])
        )
        t.start()

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
