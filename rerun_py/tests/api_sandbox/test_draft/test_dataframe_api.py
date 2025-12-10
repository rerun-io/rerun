from __future__ import annotations

import datetime
from typing import TYPE_CHECKING

import numpy as np
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from rerun_draft.catalog import DatasetEntry


def test_dataframe_api_filter_segment_id(basic_dataset: DatasetEntry) -> None:
    # Create a view with 2 segments
    view = basic_dataset.filter_segments(segment_ids=["simple_recording_0", "simple_recording_2"]).reader(
        index="timeline"
    )

    # Get dataframe from the unfiltered view and apply DataFrame-level filtering for multiple segments
    df = view.sort("rerun_segment_id")

    assert str(df) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                       │
│ * version: 0.1.2                                                                                                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────────────┬──────────────────────────────┬───────────────────────────────────┬─────────────────────────────────────────────────────┐ │
│ │ rerun_segment_id   ┆ timeline                     ┆ /points:Points2D:colors           ┆ /points:Points2D:positions                          │ │
│ │ ---                ┆ ---                          ┆ ---                               ┆ ---                                                 │ │
│ │ type: Utf8         ┆ type: nullable Timestamp(ns) ┆ type: nullable List[nullable u32] ┆ type: nullable List[nullable FixedSizeList[f32; 2]] │ │
│ │                    ┆ index_name: timeline         ┆ archetype: Points2D               ┆ archetype: Points2D                                 │ │
│ │                    ┆ kind: index                  ┆ component: Points2D:colors        ┆ component: Points2D:positions                       │ │
│ │                    ┆                              ┆ component_type: Color             ┆ component_type: Position2D                          │ │
│ │                    ┆                              ┆ entity_path: /points              ┆ entity_path: /points                                │ │
│ │                    ┆                              ┆ kind: data                        ┆ kind: data                                          │ │
│ ╞════════════════════╪══════════════════════════════╪═══════════════════════════════════╪═════════════════════════════════════════════════════╡ │
│ │ simple_recording_0 ┆ 2000-01-01T00:00:00          ┆ [4278190335, 16711935]            ┆ [[0.0, 1.0], [3.0, 4.0]]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_2 ┆ 2000-01-01T00:00:02          ┆ [4278190847, 16712447]            ┆ [[2.0, 3.0], [5.0, 6.0]]                            │ │
│ └────────────────────┴──────────────────────────────┴───────────────────────────────────┴─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    table = df.to_arrow_table()

    assert str(table) == inline_snapshot("""\
pyarrow.Table
rerun_segment_id: string not null
timeline: timestamp[ns]
/points:Points2D:colors: list<item: uint32>
  child 0, item: uint32
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
  child 0, item: fixed_size_list<item: float not null>[2]
      child 0, item: float not null
----
rerun_segment_id: [["simple_recording_0","simple_recording_2"]]
timeline: [[2000-01-01 00:00:00.000000000,2000-01-01 00:00:02.000000000]]
/points:Points2D:colors: [[[4278190335,16711935],[4278190847,16712447]]]
/points:Points2D:positions: [[[[0,1],[3,4]],[[2,3],[5,6]]]]\
""")


def test_dataframe_api_using_index_values(complex_dataset: DatasetEntry) -> None:
    dataset_view = complex_dataset.filter_segments([
        "complex_recording_0",
        "complex_recording_1",
        "complex_recording_2",
    ])

    df = dataset_view.reader(
        index="timeline",
    ).sort("rerun_segment_id", "timeline")

    assert str(df) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                                                             │
│ * version: 0.1.2                                                                                                                                                                      │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬──────────────────────────────┬───────────────────────────────────┬─────────────────────────────────────────────────────┬────────────────────────────────────┐ │
│ │ rerun_segment_id    ┆ timeline                     ┆ /points:Points2D:colors           ┆ /points:Points2D:positions                          ┆ /text:TextLog:text                 │ │
│ │ ---                 ┆ ---                          ┆ ---                               ┆ ---                                                 ┆ ---                                │ │
│ │ type: Utf8          ┆ type: nullable Timestamp(ns) ┆ type: nullable List[nullable u32] ┆ type: nullable List[nullable FixedSizeList[f32; 2]] ┆ type: nullable List[nullable Utf8] │ │
│ │                     ┆ index_name: timeline         ┆ archetype: Points2D               ┆ archetype: Points2D                                 ┆ archetype: TextLog                 │ │
│ │                     ┆ kind: index                  ┆ component: Points2D:colors        ┆ component: Points2D:positions                       ┆ component: TextLog:text            │ │
│ │                     ┆                              ┆ component_type: Color             ┆ component_type: Position2D                          ┆ component_type: Text               │ │
│ │                     ┆                              ┆ entity_path: /points              ┆ entity_path: /points                                ┆ entity_path: /text                 │ │
│ │                     ┆                              ┆ kind: data                        ┆ kind: data                                          ┆ kind: data                         │ │
│ ╞═════════════════════╪══════════════════════════════╪═══════════════════════════════════╪═════════════════════════════════════════════════════╪════════════════════════════════════╡ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:00          ┆ null                              ┆ null                                                ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:01          ┆ [4278190335, 16711935]            ┆ [[0.0, 1.0], [3.0, 4.0]]                            ┆ null                               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:02          ┆ null                              ┆ null                                                ┆ [World]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_1 ┆ 2000-01-01T00:00:01          ┆ null                              ┆ null                                                ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_1 ┆ 2000-01-01T00:00:02          ┆ [4278190591, 16712191]            ┆ [[1.0, 2.0], [4.0, 5.0]]                            ┆ null                               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_1 ┆ 2000-01-01T00:00:03          ┆ null                              ┆ null                                                ┆ [World]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:02          ┆ null                              ┆ null                                                ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:03          ┆ [4278190847, 16712447]            ┆ [[2.0, 3.0], [5.0, 6.0]]                            ┆ null                               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:04          ┆ null                              ┆ null                                                ┆ [World]                            │ │
│ └─────────────────────┴──────────────────────────────┴───────────────────────────────────┴─────────────────────────────────────────────────────┴────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    # Create a view with all partitions, applying global index values to all segments
    # Note: using_index_values now applies the same timestamps to ALL segments
    index_values = np.array(
        [
            datetime.datetime(1999, 12, 31, 23, 59, 59),
            datetime.datetime(2000, 1, 1, 0, 0, 1, microsecond=500),
            datetime.datetime(2000, 1, 1, 0, 0, 2),
        ],
        dtype=np.datetime64,
    )
    df = dataset_view.reader(
        index="timeline",
        using_index_values=index_values,
        fill_latest_at=True,
    ).sort("rerun_segment_id", "timeline")

    # Note: with global using_index_values, all 3 segments get the same 3 timestamps
    assert str(df) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                                                             │
│ * version: 0.1.2                                                                                                                                                                      │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬──────────────────────────────┬───────────────────────────────────┬─────────────────────────────────────────────────────┬────────────────────────────────────┐ │
│ │ rerun_segment_id    ┆ timeline                     ┆ /points:Points2D:colors           ┆ /points:Points2D:positions                          ┆ /text:TextLog:text                 │ │
│ │ ---                 ┆ ---                          ┆ ---                               ┆ ---                                                 ┆ ---                                │ │
│ │ type: Utf8          ┆ type: nullable Timestamp(ns) ┆ type: nullable List[nullable u32] ┆ type: nullable List[nullable FixedSizeList[f32; 2]] ┆ type: nullable List[nullable Utf8] │ │
│ │                     ┆ index_name: timeline         ┆ archetype: Points2D               ┆ archetype: Points2D                                 ┆ archetype: TextLog                 │ │
│ │                     ┆ kind: index                  ┆ component: Points2D:colors        ┆ component: Points2D:positions                       ┆ component: TextLog:text            │ │
│ │                     ┆                              ┆ component_type: Color             ┆ component_type: Position2D                          ┆ component_type: Text               │ │
│ │                     ┆                              ┆ entity_path: /points              ┆ entity_path: /points                                ┆ entity_path: /text                 │ │
│ │                     ┆                              ┆ kind: data                        ┆ kind: data                                          ┆ kind: data                         │ │
│ ╞═════════════════════╪══════════════════════════════╪═══════════════════════════════════╪═════════════════════════════════════════════════════╪════════════════════════════════════╡ │
│ │ complex_recording_0 ┆ 1999-12-31T23:59:59          ┆ null                              ┆ null                                                ┆ null                               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:01.000500   ┆ [4278190335, 16711935]            ┆ [[0.0, 1.0], [3.0, 4.0]]                            ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_0 ┆ 2000-01-01T00:00:02          ┆ [4278190335, 16711935]            ┆ [[0.0, 1.0], [3.0, 4.0]]                            ┆ [World]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_1 ┆ 1999-12-31T23:59:59          ┆ null                              ┆ null                                                ┆ null                               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_1 ┆ 2000-01-01T00:00:01.000500   ┆ null                              ┆ null                                                ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_1 ┆ 2000-01-01T00:00:02          ┆ [4278190591, 16712191]            ┆ [[1.0, 2.0], [4.0, 5.0]]                            ┆ [Hello]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 1999-12-31T23:59:59          ┆ null                              ┆ null                                                ┆ null                               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:01.000500   ┆ null                              ┆ null                                                ┆ null                               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ complex_recording_2 ┆ 2000-01-01T00:00:02          ┆ null                              ┆ null                                                ┆ [Hello]                            │ │
│ └─────────────────────┴──────────────────────────────┴───────────────────────────────────┴─────────────────────────────────────────────────────┴────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    # Verify table has the expected 9 rows (3 segments x 3 timestamps)
    table = df.to_arrow_table()
    assert len(table) == 9


def test_dataframe_api_using_index_values_empty(basic_dataset: DatasetEntry) -> None:
    """Test that empty using_index_values returns no data."""
    df = basic_dataset.reader(
        index="timeline",
        using_index_values=np.array([], dtype=np.datetime64),
        fill_latest_at=True,
    )

    assert str(df) == inline_snapshot("No data to display")

    table = df.to_arrow_table()

    assert str(table) == inline_snapshot("""\
pyarrow.Table
rerun_segment_id: string not null
timeline: timestamp[ns]
/points:Points2D:colors: list<item: uint32>
  child 0, item: uint32
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
  child 0, item: fixed_size_list<item: float not null>[2]
      child 0, item: float not null
----
rerun_segment_id: []
timeline: []
/points:Points2D:colors: []
/points:Points2D:positions: []\
""")


def test_dataframe_api_using_index_values_from_array(complex_dataset: DatasetEntry) -> None:
    """Demonstrate using numpy arrays as using_index_values."""

    # Extract specific timestamps of interest from the points data
    # These are the timestamps where points data exists
    timestamps_of_interest = np.array(
        [
            datetime.datetime(2000, 1, 1, 0, 0, 1),
            datetime.datetime(2000, 1, 1, 0, 0, 2),
            datetime.datetime(2000, 1, 1, 0, 0, 3),
        ],
        dtype=np.datetime64,
    )

    # Query the text data at those timestamps
    df = (
        complex_dataset.filter_contents(["text"])
        .reader(index="timeline", using_index_values=timestamps_of_interest)
        .sort("rerun_segment_id", "timeline")
    )

    # All segments get queried at all 3 timestamps
    table = df.to_arrow_table()
    assert len(table) == 15  # 5 segments x 3 timestamps
