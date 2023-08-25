#!/usr/bin/env python3
"""
Example template.

Run:
```sh
pip install -r examples/python/template/requirements.txt
python examples/python/template/main.py
```
"""
from __future__ import annotations

import argparse

import rerun as rr  # pip install rerun-sdk


def main() -> None:
    parser = argparse.ArgumentParser(description="Example of using the Rerun visualizer")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.rscript_setup(args, "rerun-example-my_example_name")

    # ... example code

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
