#!/usr/bin/env python3
"""Shows how rerun can work with multiprocessing."""
from __future__ import annotations

import argparse
import multiprocessing
import os
import threading

import rerun as rr  # pip install rerun-sdk


def task(child_index: int) -> None:
    # All processes spawned with `multiprocessing` will automatically
    # be assigned the same default recording_id.
    # We just need to connect each process to the the rerun viewer:
    rr.init("multiprocessing")
    rr.connect()

    title = f"task {child_index}"
    rr.log_text_entry(
        "log",
        text=f"Logging from pid={os.getpid()}, thread={threading.get_ident()} using the rerun recording id {rr.get_recording_id()}",  # noqa: E501 line too long
    )
    if child_index == 0:
        rr.log_rect(title, [5, 5, 80, 80], label=title)
    else:
        rr.log_rect(title, [10 + child_index * 10, 20 + child_index * 5, 30, 40], label=title)


def main() -> None:
    parser = argparse.ArgumentParser(description="Test multi-process logging to the same Rerun server")
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    rr.init("multiprocessing")
    rr.spawn(connect=False)  # this is the viewer that each child process will connect to

    task(0)

    # Using multiprocessing with "fork" results in a hang on shutdown so
    # always use "spawn"
    # TODO(https://github.com/rerun-io/rerun/issues/1921)
    multiprocessing.set_start_method("spawn")

    for i in [1, 2, 3]:
        p = multiprocessing.Process(target=task, args=(i,))
        p.start()
        p.join()


if __name__ == "__main__":
    main()
