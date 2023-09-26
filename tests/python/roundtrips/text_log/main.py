#!/usr/bin/env python3

"""Logs a `TextLog` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_text_log")

    rr.log("log", rr.TextLog(body="No level"))
    rr.log("log", rr.TextLog(body="INFO level", level="INFO"))
    rr.log("log", rr.TextLog(body="WILD level", level="WILD"))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
