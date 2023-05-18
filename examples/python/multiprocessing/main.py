#!/usr/bin/env python3

"""Shows how rerun can work with multiprocessing."""

import argparse
import multiprocessing
import os
import threading

import depthai_viewer as viewer


def task(title: str) -> None:
    # All processes spawned with `multiprocessing` will automatically
    # be assigned the same default recording_id.
    # We just need to connect each process to the the rerun viewer:
    viewer.connect()

    viewer.log_text_entry(
        "log",
        text=f"Logging from pid={os.getpid()}, thread={threading.get_ident()} using the rerun recording id {viewer.get_recording_id()}",  # noqa: E501 line too long
    )
    viewer.log_rect(title, [10, 20, 30, 40], label=title)


def main() -> None:
    parser = argparse.ArgumentParser(description="Test multi-process logging to the same Rerun server")
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    viewer.init("multiprocessing")
    viewer.spawn(connect=False)  # this is the viewer that each process will connect to

    task("main_task")

    # Using multiprocessing with "fork" results in a hang on shutdown so
    # always use "spawn"
    # TODO(https://github.com/rerun-io/rerun/issues/1921)
    multiprocessing.set_start_method("spawn")

    p = multiprocessing.Process(target=task, args=("child_task",))
    p.start()
    p.join()


if __name__ == "__main__":
    main()
