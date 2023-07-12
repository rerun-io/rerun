#!/usr/bin/env python3
"""Shows how rerun can work with multiprocessing."""
from __future__ import annotations

import argparse
import multiprocessing
import os
import threading

import rerun as rr  # pip install rerun-sdk


# Python does not guarantee that the normal atexit-handlers will be called at the
# termination of a multiprocessing.Process. Explicitly add the `shutdown_at_exit`
# decorator to ensure data is flushed when the task completes.
@rr.shutdown_at_exit
def task(child_index: int) -> None:
    # In the new process, we always need to call init with the same `application_id`.
    # By default, the `recording_id`` will match the `recording_id`` of the parent process,
    # so all of these processes will have their log data merged in the viewer.
    # Caution: if you manually specified `recording_id` in the parent, you also must
    # pass the same `recording_id` here.
    rr.init("multiprocessing")

    # We then have to connect to the viewer instance.
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
    parser.parse_args()

    rr.init("multiprocessing")
    rr.spawn(connect=False)  # this is the viewer that each child process will connect to

    task(0)

    for i in [1, 2, 3]:
        p = multiprocessing.Process(target=task, args=(i,))
        p.start()
        p.join()


if __name__ == "__main__":
    main()
