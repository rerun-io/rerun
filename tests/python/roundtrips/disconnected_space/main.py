#!/usr/bin/env python3

"""Logs a `DisconnectedSpace` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import rerun as rr
import rerun.experimental as rr2


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_disconnected_space")

    rr2.log("disconnected_space", rr2.DisconnectedSpace(True))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
