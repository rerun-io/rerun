#!/usr/bin/env python3

"""Logs a `VisibleTimeRanges` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import rerun as rr
import rerun.blueprint as rrb


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_visible_time_ranges")

    rr.log(
        "visible_time_ranges",
        rrb.archetypes.VisibleTimeRanges([
            rrb.VisibleTimeRange(
                "timeline0",
                start=rr.TimeRangeBoundary.infinite(),
                end=rr.TimeRangeBoundary.cursor_relative(nanos=-10),
            ),
            rrb.VisibleTimeRange(
                "timeline1",
                start=rrb.TimeRangeBoundary.cursor_relative(nanos=20),
                end=rrb.TimeRangeBoundary.infinite(),
            ),
            rrb.VisibleTimeRange(
                "timeline2",
                start=rrb.TimeRangeBoundary.absolute(nanos=20),
                end=rrb.TimeRangeBoundary.absolute(nanos=40),
            ),
        ]),
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
