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
import pathlib
import threading

import rerun as rr


def task(title: str) -> None:
    # All processes spawned with `multiprocessing` will automatically
    # be assigned the same default recording_id.
    print(
        f"Logging from pid={os.getpid()}, thread={threading.get_ident()} using the rerun recording id {rr.get_recording_id()}"
    )
    rr.connect()
    rr.log_rect(title, [10, 20, 30, 40], label=title)


def main() -> None:
    parser = argparse.ArgumentParser(description="Test multi-process logging to the same Rerun server")
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    args = parser.parse_args()

    rr.init("multiprocessing")

    if args.connect:
        task("main_task")
        p = multiprocessing.Process(target=task, args=("child_task",))
        p.start()
        p.join()
    else:
        # This is so we can run this without arguments in `just py-run-all`
        print("You must use the --connect argument!")


if __name__ == "__main__":
    main()
