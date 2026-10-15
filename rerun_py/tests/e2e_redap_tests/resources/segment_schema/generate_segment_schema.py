#!/usr/bin/env python3
"""Regenerate static segment schema resources for E2E redap tests."""

from __future__ import annotations

from pathlib import Path

import rerun as rr


def main() -> None:
    base = Path(__file__).parent

    for segment_id in ("first", "second"):
        with rr.RecordingStream("rerun_example_segment_schema", recording_id=segment_id) as rec:
            rec.save(base / f"{segment_id}.rrd")
            rec.log("/static", rr.AnyValues(value=1), static=True)
            rec.set_time(segment_id, sequence=1)
            rec.log(f"/{segment_id}", rr.AnyValues(value=2))

    with rr.RecordingStream("rerun_example_segment_schema", recording_id="first") as rec:
        rec.save(base / "predictions.rrd")
        rec.log("/predictions", rr.AnyValues(value=3), static=True)


if __name__ == "__main__":
    main()
