#!/usr/bin/env python3
"""Example of an executable data-loader plugin for the Rerun Viewer."""

from __future__ import annotations

import argparse
import os
import sys

import rerun as rr  # pip install rerun-sdk

# The Rerun Viewer will always pass these two pieces of information:
# 1. The path to be loaded, as a positional arg.
# 2. A shared recording ID, via the `--recording-id` flag.
#
# It is up to you whether you make use of that shared recording ID or not.
# If you use it, the data will end up in the same recording as all other plugins interested in
# that file, otherwise you can just create a dedicated recording for it. Or both.
#
# Check out `re_data_source::DataLoaderSettings` documentation for an exhaustive listing of
# the available CLI parameters.
parser = argparse.ArgumentParser(
    description="""
This is an example executable data-loader plugin for the Rerun Viewer.
Any executable on your `$PATH` with a name that starts with `rerun-loader-` will be
treated as an external data-loader.

This particular one will log Python source code files as markdown documents, and return a
special exit code to indicate that it doesn't support anything else.

To try it out, copy it in your $PATH as `rerun-loader-python-file`, then open a Python source
file with Rerun (`rerun file.py`).
""",
)
parser.add_argument("filepath", type=str)
parser.add_argument("--application-id", type=str, help="optional recommended ID for the application")
parser.add_argument("--recording-id", type=str, help="optional recommended ID for the recording")
parser.add_argument("--entity-path-prefix", type=str, help="optional prefix for all entity paths")
parser.add_argument("--static", action="store_true", default=False, help="optionally mark data to be logged as static")
parser.add_argument(
    "--time_sequence",
    type=str,
    action="append",
    help="optional sequences to log at (e.g. `--time_sequence sim_frame=42`)",
)
parser.add_argument(
    "--time_duration_nanos",
    type=str,
    action="append",
    help="optional duration(s) (in nanoseconds) to log at (e.g. `--time_duration_nanos sim_time=123`) (repeatable)",
)
parser.add_argument(
    "--time_timestamp_nanos",
    type=str,
    action="append",
    help="optional timestamp(s) (in nanoseconds since epochj) to log at (e.g. `--time_timestamp_nanos sim_time=1709203426123456789`) (repeatable)",
)
args = parser.parse_args()


def main() -> None:
    is_file = os.path.isfile(args.filepath)
    is_python_file = os.path.splitext(args.filepath)[1].lower() == ".py"

    # Inform the Rerun Viewer that we do not support that kind of file.
    if not is_file or not is_python_file:
        sys.exit(rr.EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE)

    app_id = "rerun_example_external_data_loader"
    if args.application_id is not None:
        app_id = args.application_id
    rr.init(app_id, recording_id=args.recording_id)
    # The most important part of this: log to standard output so the Rerun Viewer can ingest it!
    rr.stdout()

    set_time_from_args()

    if args.entity_path_prefix:
        entity_path = f"{args.entity_path_prefix}/{args.filepath}"
    else:
        entity_path = args.filepath

    with open(args.filepath, encoding="utf8") as file:
        body = file.read()
        text = f"""## Some Python code\n```python\n{body}\n```\n"""
        rr.log(entity_path, rr.TextDocument(text, media_type=rr.MediaType.MARKDOWN), static=args.static)


def set_time_from_args() -> None:
    if not args.static and args.time is not None:
        for time_str in args.time_sequence:
            parts = time_str.split("=")
            if len(parts) != 2:
                continue
            timeline_name, sequence = parts
            rr.set_time(timeline_name, sequence=int(sequence))

        for time_str in args.time_duration_nanos:
            parts = time_str.split("=")
            if len(parts) != 2:
                continue
            timeline_name, nanos = parts
            rr.set_time(timeline_name, duration=1e-9 * int(nanos))

        for time_str in args.time_timestamp_nanos:
            parts = time_str.split("=")
            if len(parts) != 2:
                continue
            timeline_name, nanos = parts
            rr.set_time(timeline_name, timestamp=1e-9 * int(nanos))


if __name__ == "__main__":
    main()
