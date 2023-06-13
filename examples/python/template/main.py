#!/usr/bin/env python3
"""
Example template.

Run:
```sh
pip install -r examples/python/template/requirements.txt
./examples/python/template/main.py
```
"""
from __future__ import annotations

import argparse
import logging

import rerun as rr  # pip install rerun-sdk


def setup_logging() -> None:
    # Forward all text logs to Rerun by attaching a handler directly to the root logger,
    # which will catch events from all loggers going forward (propagation is on by
    # default).
    #
    # For more info: https://docs.python.org/3/howto/logging.html#handlers
    logging.getLogger().addHandler(rr.log.text.LoggingHandler())
    logging.getLogger().setLevel(-1)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Describe your example here!"
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "my_example_name")

    setup_logging()

    # ... example code

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
