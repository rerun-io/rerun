from __future__ import annotations

from typing import TYPE_CHECKING

from datafusion import col
from inline_snapshot import snapshot as inline_snapshot

from .utils import segment_stable_snapshot, sorted_schema_str

if TYPE_CHECKING:
    from rerun_draft.catalog import DatasetEntry, TableEntry


def test_dataset_view_filter_segments(complex_dataset: DatasetEntry, complex_metadata: TableEntry) -> None:
    simple_filt = complex_dataset.filter_segments(["complex_recording_2"])

    assert sorted(simple_filt.segment_ids()) == inline_snapshot(["complex_recording_2"])

    assert segment_stable_snapshot(simple_filt.segment_table(join_meta=complex_metadata)) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                                             │
│ * version: 0.1.2                                                                                                                                                      │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬───────────────────┬──────────────────┬──────────────────┬──────────────────────────────┬──────────────────────────────┬─────────────────────┐ │
│ │ rerun_segment_id    ┆ rerun_layer_names ┆ rerun_num_chunks ┆ rerun_size_bytes ┆ timeline:end                 ┆ timeline:start               ┆ success             │ │
│ │ ---                 ┆ ---               ┆ ---              ┆ ---              ┆ ---                          ┆ ---                          ┆ ---                 │ │
│ │ type: Utf8          ┆ type: List[Utf8]  ┆ type: u64        ┆ type: u64        ┆ type: nullable Timestamp(ns) ┆ type: nullable Timestamp(ns) ┆ type: nullable bool │ │
│ │                     ┆                   ┆                  ┆                  ┆ index: timeline              ┆ index: timeline              ┆                     │ │
│ │                     ┆                   ┆                  ┆                  ┆ index_kind: timestamp        ┆ index_kind: timestamp        ┆                     │ │
│ │                     ┆                   ┆                  ┆                  ┆ index_marker: end            ┆ index_marker: start          ┆                     │ │
│ │                     ┆                   ┆                  ┆                  ┆ kind: index                  ┆ kind: index                  ┆                     │ │
│ ╞═════════════════════╪═══════════════════╪══════════════════╪══════════════════╪══════════════════════════════╪══════════════════════════════╪═════════════════════╡ │
│ │ complex_recording_2 ┆ [base]            ┆ 3                ┆ 4226             ┆ 2000-01-01T00:00:04          ┆ 2000-01-01T00:00:02          ┆ false               │ │
│ └─────────────────────┴───────────────────┴──────────────────┴──────────────────┴──────────────────────────────┴──────────────────────────────┴─────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    good_segments = complex_dataset.segment_table(join_meta=complex_metadata).filter(col("success"))

    good_ds = complex_dataset.filter_segments(segment_ids=good_segments)

    assert sorted(good_ds.segment_ids()) == inline_snapshot(["complex_recording_1", "complex_recording_3"])

    assert segment_stable_snapshot(good_ds.segment_table()) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                       │
│ * version: 0.1.2                                                                                                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬───────────────────┬──────────────────┬──────────────────┬──────────────────────────────┬──────────────────────────────┐ │
│ │ rerun_segment_id    ┆ rerun_layer_names ┆ rerun_num_chunks ┆ rerun_size_bytes ┆ timeline:end                 ┆ timeline:start               │ │
│ │ ---                 ┆ ---               ┆ ---              ┆ ---              ┆ ---                          ┆ ---                          │ │
│ │ type: Utf8          ┆ type: List[Utf8]  ┆ type: u64        ┆ type: u64        ┆ type: nullable Timestamp(ns) ┆ type: nullable Timestamp(ns) │ │
│ │                     ┆                   ┆                  ┆                  ┆ index: timeline              ┆ index: timeline              │ │
│ │                     ┆                   ┆                  ┆                  ┆ index_kind: timestamp        ┆ index_kind: timestamp        │ │
│ │                     ┆                   ┆                  ┆                  ┆ index_marker: end            ┆ index_marker: start          │ │
│ │                     ┆                   ┆                  ┆                  ┆ kind: index                  ┆ kind: index                  │ │
│ ╞═════════════════════╪═══════════════════╪══════════════════╪══════════════════╪══════════════════════════════╪══════════════════════════════╡ │
│ │ complex_recording_1 ┆ [base]            ┆ 3                ┆ 4226             ┆ 2000-01-01T00:00:03          ┆ 2000-01-01T00:00:01          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_3 ┆ [base]            ┆ 3                ┆ 4226             ┆ 2000-01-01T00:00:05          ┆ 2000-01-01T00:00:03          │ │
│ └─────────────────────┴───────────────────┴──────────────────┴──────────────────┴──────────────────────────────┴──────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_dataset_view_filter_entities(complex_dataset: DatasetEntry) -> None:
    assert sorted_schema_str(complex_dataset.arrow_schema()) == inline_snapshot("""\
/points:Points2D:colors: list<item: uint32>
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
/text:TextLog:text: list<item: string>
property:RecordingInfo:start_time: list<item: int64>
rerun.controls.RowId: fixed_size_binary[16]
timeline: timestamp[ns]\
""")

    entity_filt = complex_dataset.filter_contents(["/points/**"])

    assert sorted_schema_str(entity_filt.arrow_schema()) == inline_snapshot("""\
/points:Points2D:colors: list<item: uint32>
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
rerun.controls.RowId: fixed_size_binary[16]
timeline: timestamp[ns]\
""")


def test_dataset_view_schema(complex_dataset: DatasetEntry) -> None:
    entity_filt = complex_dataset.filter_contents(["/points/**"])

    assert str(entity_filt.schema()) == inline_snapshot("""\
Index(timeline:timeline)
Column name: /points:Points2D:colors
	Entity path: /points
	Archetype: rerun.archetypes.Points2D
	Component type: rerun.components.Color
	Component: Points2D:colors
Column name: /points:Points2D:positions
	Entity path: /points
	Archetype: rerun.archetypes.Points2D
	Component type: rerun.components.Position2D
	Component: Points2D:positions\
""")

    assert entity_filt.schema().column_names() == inline_snapshot([
        "timeline",
        "/points:Points2D:colors",
        "/points:Points2D:positions",
    ])


def test_dataset_view_dataframe(complex_dataset: DatasetEntry) -> None:
    filtered = complex_dataset.filter_contents(["/text/**"]).filter_segments([
        "complex_recording_0",
        "complex_recording_2",
    ])

    schema = filtered.arrow_schema()

    assert sorted_schema_str(schema) == inline_snapshot("""\
/text:TextLog:text: list<item: string>
rerun.controls.RowId: fixed_size_binary[16]
timeline: timestamp[ns]\
""")

    df = filtered.reader(index="timeline").sort("rerun_segment_id")

    assert str(df) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                   │
│ * version: 0.1.2                                                                            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬──────────────────────────────┬────────────────────────────────────┐ │
│ │ rerun_segment_id    ┆ timeline                     ┆ /text:TextLog:text                 │ │
│ │ ---                 ┆ ---                          ┆ ---                                │ │
│ │ type: Utf8          ┆ type: nullable Timestamp(ns) ┆ type: nullable List[nullable Utf8] │ │
│ │                     ┆ index_name: timeline         ┆ archetype: TextLog                 │ │
│ │                     ┆ kind: index                  ┆ component: TextLog:text            │ │
│ │                     ┆                              ┆ component_type: Text               │ │
│ │                     ┆                              ┆ entity_path: /text                 │ │
│ │                     ┆                              ┆ kind: data                         │ │
│ ╞═════════════════════╪══════════════════════════════╪════════════════════════════════════╡ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:00          ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:02          ┆ [World]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:02          ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:04          ┆ [World]                            │ │
│ └─────────────────────┴──────────────────────────────┴────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────┘\
""")
