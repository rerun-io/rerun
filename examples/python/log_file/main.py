#!/usr/bin/env python3
"""
Demonstrates how to log any file from the SDK using the `DataLoader` machinery.

See <https://www.rerun.io/docs/howto/open-any-file> for more information.

Usage:
```
python examples/python/log_file/main.py -- examples/assets
```
"""
from __future__ import annotations

import argparse
from pathlib import Path

import rerun as rr  # pip install rerun-sdk

parser = argparse.ArgumentParser(
    description="Demonstrates how to log any file from the SDK using the `DataLoader` machinery."
)
rr.script_add_args(parser)
parser.add_argument(
    "--from-contents",
    action="store_true",
    default=False,
    help="Log the contents of the file directly (files only -- not supported by external loaders).",
)
parser.add_argument("filepaths", nargs="+", type=Path, help="The filepaths to be loaded and logged.")
args = parser.parse_args()

rr.script_setup(args, "rerun_example_log_file")

for filepath in args.filepaths:
    if not args.from_contents:
        # Either log the file using its path…
        rr.log_file_from_path(filepath, entity_path_prefix="log_file_example")
    else:
        # …or using its contents if you already have them loaded for some reason.
        try:
            with open(filepath, "rb") as file:
                rr.log_file_from_contents(filepath, file.read(), entity_path_prefix="log_file_example")
        except Exception:
            pass

rr.script_teardown(args)
