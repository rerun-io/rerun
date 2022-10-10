#!/usr/bin/env python3

import multiprocessing
import os
import threading

import rerun_sdk as rerun


def task(title):
    # All processes spawned with `multiprocessing` will automatically
    # be assigned the same default recording_id.
    print(
        f"Logging from pid={os.getpid()}, thread={threading.get_ident()} using the rerun recording id {rerun.get_recording_id()}"
    )
    rerun.connect()
    rerun.log_rect(title, [10, 20, 30, 40], label=title, space=title)


def main() -> None:
    task("main_task")
    p = multiprocessing.Process(target=task, args=("child_task",))
    p.start()
    p.join()


if __name__ == "__main__":
    main()
