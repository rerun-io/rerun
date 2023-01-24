#!/usr/bin/env python3

"""
For this example, first start `rerun`, then run this example:

```sh
cargo r &
examples/multiprocessing/main.py --connect
```
"""

import argparse
import multiprocessing
import os
import threading

import rerun as rr


def task(title: str) -> None:
    # All processes spawned with `multiprocessing` will automatically
    # be assigned the same default recording_id.
    rr.connect()
    rr.log_text_entry("log",
        text=f"Logging from pid={os.getpid()}, thread={threading.get_ident()} using the rerun recording id {rr.get_recording_id()}"
    )
    rr.log_rect(title, [10, 20, 30, 40], label=title)


def main() -> None:
    parser = argparse.ArgumentParser(description="Test multi-process logging to the same Rerun server")
    args = parser.parse_args()

    rr.init("multiprocessing")
    rr.spawn_viewer()

    task("main_task")

    p = multiprocessing.Process(target=task, args=("child_task",))
    p.start()
    p.join()


if __name__ == "__main__":
    main()
