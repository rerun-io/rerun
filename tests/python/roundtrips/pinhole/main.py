#!/usr/bin/env python3

"""Logs a `Pinhole` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_pinhole")

    rr.log(
        "pinhole",
        rr.Pinhole(image_from_camera=[[3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [1.5, 1.5, 1.0]], resolution=[3840, 2160]),
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
