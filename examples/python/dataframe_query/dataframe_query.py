#!/usr/bin/env python3
"""Demonstrates basic usage of the dataframe APIs."""

from __future__ import annotations

import argparse

import pyarrow as pa
import rerun as rr

DESCRIPTION = """
Usage: python dataframe_query.py <path_to_rrd> [entity_path_filter]

This example will query for the first 10 rows of data in your recording of choice,
and display the results as a table in your terminal.

You can use one of your recordings, or grab one from our hosted examples, e.g.:
  curl 'https://app.rerun.io/version/latest/examples/dna.rrd' -o - > /tmp/dna.rrd

The results can be filtered further by specifying an entity filter expression:
  {bin_name} my_recording.rrd /helix/structure/**
""".strip()


def query(path_to_rrd: str, entity_path_filter: str) -> None:
    recording = rr.dataframe.load_recording(path_to_rrd)
    view = recording.view(index="log_time", contents=entity_path_filter)
    batches = view.select()

    table = pa.Table.from_batches(batches, batches.schema)
    table = table.slice(0, 10)
    print(table.to_pandas())


def main() -> None:
    parser = argparse.ArgumentParser(description=DESCRIPTION)
    parser.add_argument("path_to_rrd", type=str, help="Path to the .rrd file")
    parser.add_argument(
        "entity_path_filter",
        type=str,
        nargs="?",
        default="/**",
        help="Optional entity path filter expression",
    )
    args = parser.parse_args()

    query(args.path_to_rrd, args.entity_path_filter)


if __name__ == "__main__":
    main()
