#!/usr/bin/env python3

"""Logs a `ViewCoordinates` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_view_coordinates")

    rr.log("/", rr.ViewCoordinates.RDF, static=True)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
