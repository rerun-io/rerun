#!/usr/bin/env python3
"""
Example template.

Run:
```sh
./examples/python/template/main.py
```
"""
from __future__ import annotations

import argparse
import logging

import rerun as rr  # pip install rerun-sdk


def setup_logging() -> None:
    # That's really all there is to it: attach a Rerun logging handler to one
    # or more loggers of your choosing and your logs will be forwarded.
    #
    # In this case we attach our handler directly to the root logger, which
    # will catch events from all loggers going forward (propagation is on by
    # default).
    #
    # For more info: https://docs.python.org/3/howto/logging.html#handlers
    logging.getLogger().addHandler(rr.log.text.LoggingHandler())
    logging.getLogger().setLevel(-1)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="demonstrates how to integrate python's native `logging` with the Rerun SDK"
    )
    rr.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    rr.script_setup(args, "text_logging")

    setup_logging()

    # ... example code

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
