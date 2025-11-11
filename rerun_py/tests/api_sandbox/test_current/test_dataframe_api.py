from __future__ import annotations

import datetime
from typing import TYPE_CHECKING

import numpy as np
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_dataframe_api_filter_partition_id(simple_dataset_prefix: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_dataset_prefix}) as server:
        client = server.client()
        ds = client.get_dataset_entry(name="ds")

        # Create a view with all partitions
        view = ds.dataframe_query_view(index="timeline", contents="/**").filter_partition_id(
            "simple_recording_0", "simple_recording_2"
        )

        # Get dataframe from the unfiltered view and apply DataFrame-level filtering for multiple partitions
        df = view.df().sort("rerun_partition_id")

        assert str(df) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                       │
│ * version: 0.1.1                                                                                                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────────────┬──────────────────────────────┬───────────────────────────────────┬─────────────────────────────────────────────────────┐ │
│ │ rerun_partition_id ┆ timeline                     ┆ /points:Points2D:colors           ┆ /points:Points2D:positions                          │ │
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
rerun_partition_id: string not null
timeline: timestamp[ns]
/points:Points2D:colors: list<item: uint32>
  child 0, item: uint32
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
  child 0, item: fixed_size_list<item: float not null>[2]
      child 0, item: float not null
----
rerun_partition_id: [["simple_recording_0","simple_recording_2"]]
timeline: [[2000-01-01 00:00:00.000000000,2000-01-01 00:00:02.000000000]]
/points:Points2D:colors: [[[4278190335,16711935],[4278190847,16712447]]]
/points:Points2D:positions: [[[[0,1],[3,4]],[[2,3],[5,6]]]]\
""")


def test_dataframe_api_using_index_values(simple_dataset_prefix: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_dataset_prefix}) as server:
        client = server.client()
        ds = client.get_dataset_entry(name="ds")

        # Create a view with all partitions
        view = (
            ds.dataframe_query_view(index="timeline", contents="/**")
            .using_index_values(
                np.array(
                    [
                        datetime.datetime(1999, 12, 31, 23, 59, 59),
                        datetime.datetime(2000, 1, 1, 0, 0, 1, microsecond=500),
                        datetime.datetime(2000, 1, 1, 0, 0, 6),
                    ],
                    dtype=np.datetime64,
                )
            )
            .fill_latest_at()
        )

        df = view.df().sort("rerun_partition_id", "timeline")

        assert str(df) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                       │
│ * version: 0.1.1                                                                                                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────────────┬──────────────────────────────┬───────────────────────────────────┬─────────────────────────────────────────────────────┐ │
│ │ rerun_partition_id ┆ timeline                     ┆ /points:Points2D:colors           ┆ /points:Points2D:positions                          │ │
│ │ ---                ┆ ---                          ┆ ---                               ┆ ---                                                 │ │
│ │ type: Utf8         ┆ type: nullable Timestamp(ns) ┆ type: nullable List[nullable u32] ┆ type: nullable List[nullable FixedSizeList[f32; 2]] │ │
│ │                    ┆ index_name: timeline         ┆ archetype: Points2D               ┆ archetype: Points2D                                 │ │
│ │                    ┆ kind: index                  ┆ component: Points2D:colors        ┆ component: Points2D:positions                       │ │
│ │                    ┆                              ┆ component_type: Color             ┆ component_type: Position2D                          │ │
│ │                    ┆                              ┆ entity_path: /points              ┆ entity_path: /points                                │ │
│ │                    ┆                              ┆ kind: data                        ┆ kind: data                                          │ │
│ ╞════════════════════╪══════════════════════════════╪═══════════════════════════════════╪═════════════════════════════════════════════════════╡ │
│ │ simple_recording_0 ┆ 1999-12-31T23:59:59          ┆ null                              ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_0 ┆ 2000-01-01T00:00:01.000500   ┆ [4278190335, 16711935]            ┆ [[0.0, 1.0], [3.0, 4.0]]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_0 ┆ 2000-01-01T00:00:06          ┆ [4278190335, 16711935]            ┆ [[0.0, 1.0], [3.0, 4.0]]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_1 ┆ 1999-12-31T23:59:59          ┆ null                              ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_1 ┆ 2000-01-01T00:00:01.000500   ┆ [4278190591, 16712191]            ┆ [[1.0, 2.0], [4.0, 5.0]]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_1 ┆ 2000-01-01T00:00:06          ┆ [4278190591, 16712191]            ┆ [[1.0, 2.0], [4.0, 5.0]]                            │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_2 ┆ 1999-12-31T23:59:59          ┆ null                              ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_2 ┆ 2000-01-01T00:00:01.000500   ┆ null                              ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_2 ┆ 2000-01-01T00:00:06          ┆ [4278190847, 16712447]            ┆ [[2.0, 3.0], [5.0, 6.0]]                            │ │
│ └────────────────────┴──────────────────────────────┴───────────────────────────────────┴─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

        table = df.to_arrow_table()

        assert str(table) == inline_snapshot("""\
pyarrow.Table
rerun_partition_id: string not null
timeline: timestamp[ns]
/points:Points2D:colors: list<item: uint32>
  child 0, item: uint32
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
  child 0, item: fixed_size_list<item: float not null>[2]
      child 0, item: float not null
----
rerun_partition_id: [["simple_recording_0","simple_recording_0","simple_recording_0","simple_recording_1","simple_recording_1","simple_recording_1","simple_recording_2","simple_recording_2","simple_recording_2"]]
timeline: [[1999-12-31 23:59:59.000000000,2000-01-01 00:00:01.000500000,2000-01-01 00:00:06.000000000,1999-12-31 23:59:59.000000000,2000-01-01 00:00:01.000500000,2000-01-01 00:00:06.000000000,1999-12-31 23:59:59.000000000,2000-01-01 00:00:01.000500000,2000-01-01 00:00:06.000000000]]
/points:Points2D:colors: [[null,[4278190335,16711935],...,null,[4278190847,16712447]]]
/points:Points2D:positions: [[null,[[0,1],[3,4]],...,null,[[2,3],[5,6]]]]\
""")
