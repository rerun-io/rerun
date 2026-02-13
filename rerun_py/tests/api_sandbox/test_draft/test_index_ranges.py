from __future__ import annotations

from typing import TYPE_CHECKING

from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from rerun_draft.catalog import DatasetEntry


def test_index_ranges(complex_dataset: DatasetEntry) -> None:
    df = complex_dataset.get_index_ranges().sort("rerun_segment_id")

    assert str(df) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                             │
│ * version: 0.1.2                                                                      │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬──────────────────────────────┬──────────────────────────────┐ │
│ │ rerun_segment_id    ┆ timeline:start               ┆ timeline:end                 │ │
│ │ ---                 ┆ ---                          ┆ ---                          │ │
│ │ type: Utf8          ┆ type: nullable Timestamp(ns) ┆ type: nullable Timestamp(ns) │ │
│ │                     ┆ index: timeline              ┆ index: timeline              │ │
│ │                     ┆ index_kind: timestamp        ┆ index_kind: timestamp        │ │
│ │                     ┆ index_marker: start          ┆ index_marker: end            │ │
│ │                     ┆ kind: index                  ┆ kind: index                  │ │
│ ╞═════════════════════╪══════════════════════════════╪══════════════════════════════╡ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:00          ┆ 2000-01-01T00:00:02          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_1 ┆ 2000-01-01T00:00:01          ┆ 2000-01-01T00:00:03          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:02          ┆ 2000-01-01T00:00:04          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_3 ┆ 2000-01-01T00:00:03          ┆ 2000-01-01T00:00:05          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_4 ┆ 2000-01-01T00:00:04          ┆ 2000-01-01T00:00:06          │ │
│ └─────────────────────┴──────────────────────────────┴──────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_index_ranges_dataset_view(complex_dataset: DatasetEntry) -> None:
    view = complex_dataset.filter_segments(["complex_recording_0", "complex_recording_1"])

    df = view.get_index_ranges().sort("rerun_segment_id")

    assert str(df) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                             │
│ * version: 0.1.2                                                                      │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬──────────────────────────────┬──────────────────────────────┐ │
│ │ rerun_segment_id    ┆ timeline:start               ┆ timeline:end                 │ │
│ │ ---                 ┆ ---                          ┆ ---                          │ │
│ │ type: Utf8          ┆ type: nullable Timestamp(ns) ┆ type: nullable Timestamp(ns) │ │
│ │                     ┆ index: timeline              ┆ index: timeline              │ │
│ │                     ┆ index_kind: timestamp        ┆ index_kind: timestamp        │ │
│ │                     ┆ index_marker: start          ┆ index_marker: end            │ │
│ │                     ┆ kind: index                  ┆ kind: index                  │ │
│ ╞═════════════════════╪══════════════════════════════╪══════════════════════════════╡ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:00          ┆ 2000-01-01T00:00:02          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_1 ┆ 2000-01-01T00:00:01          ┆ 2000-01-01T00:00:03          │ │
│ └─────────────────────┴──────────────────────────────┴──────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────┘\
""")

    # Note: the ranges are relative to the actual data being filtered, see how the ranges differ here

    view = view.filter_contents(["/points"])
    df = view.get_index_ranges().sort("rerun_segment_id")

    assert str(df) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                             │
│ * version: 0.1.2                                                                      │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬──────────────────────────────┬──────────────────────────────┐ │
│ │ rerun_segment_id    ┆ timeline:start               ┆ timeline:end                 │ │
│ │ ---                 ┆ ---                          ┆ ---                          │ │
│ │ type: Utf8          ┆ type: nullable Timestamp(ns) ┆ type: nullable Timestamp(ns) │ │
│ │                     ┆ index: timeline              ┆ index: timeline              │ │
│ │                     ┆ index_kind: timestamp        ┆ index_kind: timestamp        │ │
│ │                     ┆ index_marker: start          ┆ index_marker: end            │ │
│ │                     ┆ kind: index                  ┆ kind: index                  │ │
│ ╞═════════════════════╪══════════════════════════════╪══════════════════════════════╡ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:00          ┆ 2000-01-01T00:00:02          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_1 ┆ 2000-01-01T00:00:01          ┆ 2000-01-01T00:00:03          │ │
│ └─────────────────────┴──────────────────────────────┴──────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────┘\
""")
