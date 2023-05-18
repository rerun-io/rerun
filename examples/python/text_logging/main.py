#!/usr/bin/env python3

"""
How to integrate python's native `logging` with the Rerun SDK.

Run:
```sh
./examples/python/text_logging/main.py
```
"""


import argparse
import logging

import depthai_viewer as viewer


def setup_logging() -> None:
    # That's really all there is to it: attach a Rerun logging handler to one
    # or more loggers of your choosing and your logs will be forwarded.
    #
    # In this case we attach our handler directly to the root logger, which
    # will catch events from all loggers going forward (propagation is on by
    # default).
    #
    # For more info: https://docs.python.org/3/howto/logging.html#handlers
    logging.getLogger().addHandler(viewer.log.text.LoggingHandler())
    logging.getLogger().setLevel(-1)


def log_stuff(frame_offset: int) -> None:
    # The usual
    logging.critical("catastrophic failure")
    logging.error("not going too well")
    logging.info("somewhat relevant")
    logging.debug("potentially interesting")

    # Custom log levels
    logging.addLevelName(42, "IMPORTANT")
    logging.log(42, "end-user deemed this important")

    # Log anything
    logging.info("here's some data: %s", {"some": 42, "data": True})

    # Log multi-line text
    logging.info("First line\nSecond line\nAnd third!")
    # Log multi-line text using the evil \r
    logging.info("Line ending with \\r\\n\r\nSecond line ending with \\n\\r\n\rAnd a third line, which just ends")

    # Test that we can log multiple times to the same sequence timeline and still
    # have the log messages show up in the correct chronological order in the viewer:
    for frame_nr in range(2):
        viewer.set_time_sequence("frame_nr", 2 * frame_offset + frame_nr)
        logging.info(f"Log one thing during frame {frame_nr}")
        logging.info(f"Log second thing during the same frame {frame_nr}")
        logging.info(f"Log third thing during the same frame {frame_nr}")

    # Use child loggers to map to arbitrary entity paths
    inner_logger = logging.getLogger("foo.bar.baz")
    inner_logger.info("hey")

    # Use spaces to create distinct logging streams
    other_logger = logging.getLogger("totally.unrelated")
    other_logger.propagate = False  # don't want root logger to catch those
    other_logger.addHandler(viewer.log.text.LoggingHandler("3rd_party_logs"))
    for _ in range(10):
        other_logger.debug("look ma, got my very own view!")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="demonstrates how to integrate python's native `logging` with the Rerun SDK"
    )
    parser.add_argument("--repeat", type=int, default=1, help="How many times do we want to run the log function?")
    viewer.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    viewer.script_setup(args, "text_logging")

    setup_logging()
    for frame_offset in range(args.repeat):
        log_stuff(frame_offset)

    viewer.script_teardown(args)


if __name__ == "__main__":
    main()
