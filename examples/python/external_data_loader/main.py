#!/usr/bin/env python3
"""Example of an executable data-loader plugin for the Rerun Viewer."""
from __future__ import annotations

import argparse
import os

import rerun as rr  # pip install rerun-sdk

# The Rerun Viewer will always pass these two pieces of information:
# 1. The path to be loaded, as a positional arg.
# 2. A shared recording ID, via the `--recording-id` flag.
#
# It is up to you whether you make use of that shared recording ID or not.
# If you use it, the data will end up in the same recording as all other plugins interested in
# that file, otherwise you can just create a dedicated recording for it. Or both.
parser = argparse.ArgumentParser(
    description="""
This is an example executable data-loader plugin for the Rerun Viewer.

It will log Python source code files as markdown documents.
To try it out, copy it in your $PATH as `rerun-loader-python-file`, then open a Python source file with Rerun (`rerun file.py`).
"""
)
parser.add_argument("filepath", type=str)
parser.add_argument("--recording-id", type=str)
args = parser.parse_args()


def main() -> None:
    is_file = os.path.isfile(args.filepath)
    is_python_file = os.path.splitext(args.filepath)[1].lower() == ".py"

    # Inform the Rerun Viewer that we do not support that kind of file.
    if not is_file or not is_python_file:
        exit(rr.EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE)

    rr.init("rerun_example_external_data_loader", recording_id=args.recording_id)
    # The most important part of this: log to standard output so the Rerun Viewer can ingest it!
    rr.stdout()

    with open(args.filepath) as file:
        body = file.read()
        text = f"""## Some Python code\n```python\n{body}\n```\n"""
        rr.log(args.filepath, rr.TextDocument(text, media_type=rr.MediaType.MARKDOWN), timeless=True)


if __name__ == "__main__":
    main()
