#!/usr/bin/env python3

"""Demonstration of using rerun from multiple threads."""

import argparse
import random
import threading

import numpy as np
import numpy.typing as npt
import depthai_viewer as viewer


def rect_logger(path: str, color: npt.NDArray[np.float32]) -> None:
    for _ in range(1000):
        rects_xy = np.random.rand(5, 2) * 1024
        rects_wh = np.random.rand(5, 2) * (1024 - rects_xy + 1)
        rects = np.hstack((rects_xy, rects_wh))
        viewer.log_rects(path, rects, colors=color, rect_format=viewer.RectFormat.XYWH)


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    viewer.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    viewer.script_setup(args, "multithreading")

    threads = []
    for i in range(10):
        t = threading.Thread(
            target=rect_logger, args=("thread/{}".format(i), [random.randrange(255) for _ in range(3)])
        )
        t.start()
        threads.append(t)

    for t in threads:
        t.join()

    viewer.script_teardown(args)


if __name__ == "__main__":
    main()
