#!/usr/bin/env python3

"""
This example demonstrates how to integrate python's native `logging` with the Rerun SDK.

Run:
```sh
./examples/logging/main.py
```
"""


import argparse
import logging

import rerun_sdk as rerun

from rerun_sdk import RerunHandler


def log_stuff():
    # That's really all there is to it: attach a Rerun logging handler to one
    # or more loggers of your choosing and your logs will be forwarded.
    #
    # In this case we attach our handler directly to the root logger, which
    # will catch events from all loggers going forward (propagation is on by
    # default).
    #
    # For more info: https://docs.python.org/3/howto/logging.html#handlers
    logging.getLogger().addHandler(RerunHandler())
    logging.getLogger().setLevel(-1)

    # The usual
    logging.critical("catastrophic failure")
    logging.error("not going too well")
    logging.info("somewhat relevant")
    logging.debug("potentially interesting")

    # Custom log levels
    logging.addLevelName(42, "IMPORTANT")
    logging.log(42, "end-user deemed this important")

    # Log anything
    logging.info("here's some data: %s", { "some": 42, "data": True })

    # Use child loggers to map to arbitrary object paths
    inner_logger = logging.getLogger("foo.bar.baz")
    inner_logger.info("hey")

    # Use spaces to create distinct logging streams
    other_logger = logging.getLogger("totally.unrelated")
    other_logger.propagate = False # don't want root logger to catch those
    other_logger.addHandler(RerunHandler("3rd-party logs"))
    for _ in range(10):
        other_logger.debug("look ma, got my very own window!")


def main():
    parser = argparse.ArgumentParser(
        description="demonstrates how to integrate python's native `logging` with the Rerun SDK")
    parser.add_argument('--headless', action='store_true',
                        help="Don't show GUI")
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    parser.add_argument('--addr', type=str, default=None,
                        help='Connect to this ip:port')
    parser.add_argument('--save', type=str, default=None,
                        help='Save data to a .rrd file at this path')
    parser.add_argument(
        "--serve",
        dest="serve",
        action="store_true",
        help="Serve a web viewer (WARNING: experimental feature)",
    )
    args = parser.parse_args()

    if args.serve:
        rerun.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    log_stuff()

    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            from time import sleep
            sleep(100_000)
        except:
            pass
    elif args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()


if __name__ == '__main__':
    main()
