from __future__ import annotations

import numpy as np
from inline_snapshot import snapshot
from rerun.catalog import DatasetInfo, LayerInfo, TimelineInfo


def _info(**overrides: object) -> DatasetInfo:
    fields: dict[str, object] = {
        "dataset_name": "robot_runs",
        "num_segments": 120,
        "size_bytes": 410_602_113_024,
        "layers": frozenset({
            LayerInfo(name="predictions-v1", num_segments=118),
            LayerInfo(name="base", num_segments=120),
            LayerInfo(name="annotations", num_segments=1),
        }),
        "timelines": frozenset({
            TimelineInfo(
                name="log_time",
                kind="timestamp",
                num_segments=120,
                min=np.datetime64("2026-09-01T08:12:03.120000000", "ns"),
                max=np.datetime64("2026-09-30T17:45:59.004000000", "ns"),
            ),
            TimelineInfo(name="step", kind="sequence", num_segments=120, min=np.int64(0), max=np.int64(184)),
            TimelineInfo(
                name="sim_time",
                kind="duration",
                num_segments=3,
                min=np.timedelta64(1_500_000_000, "ns"),
                max=np.timedelta64(90_000_000_000, "ns"),
            ),
            TimelineInfo(name="frame_nr", kind="sequence", num_segments=0, min=None, max=None),
        }),
        "num_entities": 37,
        "num_component_columns": 112,
        "property_columns": frozenset({"property:episode:success", "property:episode:operator"}),
    }
    fields.update(overrides)
    return DatasetInfo(**fields)  # type: ignore[arg-type]


def test_repr() -> None:
    assert repr(_info()) == snapshot("""\
DatasetInfo(robot_runs: 120 segments, 382 GiB)
  contents: 37 entities, 112 component columns
  properties: property:episode:operator, property:episode:success
  layers:
    annotations: in 1 segment
    base: in 120 segments
    predictions-v1: in 118 segments
  timelines:
    frame_nr: sequence, no data
    log_time: timestamp, 2026-09-01 08:12:03.120Z … 2026-09-30 17:45:59.004Z, in 120 segments
    sim_time: duration, +1.5s … +90s, in 3 segments
    step: sequence, #0 … #184, in 120 segments\
""")


def test_repr_empty() -> None:
    info = _info(
        num_segments=0,
        size_bytes=0,
        layers=frozenset(),
        timelines=frozenset(),
        property_columns=frozenset(),
    )
    assert repr(info) == snapshot("""\
DatasetInfo(robot_runs: 0 segments, 0 B)
  contents: 37 entities, 112 component columns
  properties: (none)\
""")


def test_child_reprs() -> None:
    assert repr(LayerInfo(name="base", num_segments=120)) == snapshot("LayerInfo(base: in 120 segments)")
    assert repr(TimelineInfo(name="frame_nr", kind="sequence", num_segments=0, min=None, max=None)) == snapshot(
        "TimelineInfo(frame_nr: sequence, no data)"
    )


def test_is_hashable() -> None:
    hash(_info())
