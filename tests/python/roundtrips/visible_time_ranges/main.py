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

    # TODO(#6221): There's improvements pending for this api.
    rr.log(
        "visible_time_ranges",
        rrb.archetypes.VisibleTimeRanges([
            rr.VisibleTimeRange(
                "timeline0",
                rr.TimeRange(rr.TimeRangeBoundary(None, "infinite"), rr.TimeRangeBoundary(-10, "cursor_relative")),
            ),
            rr.VisibleTimeRange(
                "timeline1",
                rr.TimeRange(rr.TimeRangeBoundary(20, "cursor_relative"), rr.TimeRangeBoundary(None, "infinite")),
            ),
            rr.VisibleTimeRange(
                "timeline2", rr.TimeRange(rr.TimeRangeBoundary(20, "absolute"), rr.TimeRangeBoundary(40, "absolute"))
            ),
        ]),
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
