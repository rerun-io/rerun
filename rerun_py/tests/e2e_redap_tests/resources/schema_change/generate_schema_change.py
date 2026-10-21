#!/usr/bin/env python3
"""Regenerate the schema-change resource for E2E redap tests."""

from __future__ import annotations

from pathlib import Path

import rerun as rr

# Registered and unregistered by the schema-cache tests, so the segment ids must stay stable.
BOXES_RECORDING_ID = "e2e_schema_change_boxes"
NEW_INDEX_RECORDING_ID = "e2e_schema_change_new_index"

# A timeline that no `dataset/` file uses, so registering `new_index.rrd` adds an index to the dataset schema.
NEW_INDEX = "time_schema_change"


def main() -> None:
    here = Path(__file__).parent

    with rr.RecordingStream("rerun_example_e2e_schema_change", recording_id=BOXES_RECORDING_ID) as rec:
        rec.save(here / "boxes.rrd")
        for i in range(3):
            rec.set_time("time_3", sequence=i + 1)
            rec.log("/boxes", rr.Boxes2D(half_sizes=[[1.0, 1.0]]))
        rec.flush()

    with rr.RecordingStream("rerun_example_e2e_schema_change", recording_id=NEW_INDEX_RECORDING_ID) as rec:
        rec.save(here / "new_index.rrd")
        for i in range(3):
            rec.set_time(NEW_INDEX, sequence=i + 1)
            rec.log("/points", rr.Points2D(positions=[[1.0, 1.0]]))
        rec.flush()


if __name__ == "__main__":
    main()
