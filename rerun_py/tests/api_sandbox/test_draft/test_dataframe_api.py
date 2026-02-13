from __future__ import annotations

from typing import TYPE_CHECKING

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
