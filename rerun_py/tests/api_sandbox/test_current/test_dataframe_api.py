from __future__ import annotations

from typing import TYPE_CHECKING

import rerun as rr
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_dataframe_api_with_local_server(simple_recording_path: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_recording_path.parent}) as server:
        client = server.client()
        ds = client.get_dataset_entry(name="ds")

        view = ds.dataframe_query_view(index="timeline", contents="/**")

        df = view.df()

        assert str(df) == inline_snapshot("""\
┌─────────────────────┬──────────────────────────────┬───────────────────────────────────┬─────────────────────────────────────────────────────┐
│ rerun_partition_id  ┆ timeline                     ┆ /points:Points2D:colors           ┆ /points:Points2D:positions                          │
│ ---                 ┆ ---                          ┆ ---                               ┆ ---                                                 │
│ type: Utf8          ┆ type: nullable Timestamp(ns) ┆ type: nullable List[nullable u32] ┆ type: nullable List[nullable FixedSizeList[f32; 2]] │
│                     ┆ index_name: timeline         ┆ archetype: Points2D               ┆ archetype: Points2D                                 │
│                     ┆ kind: index                  ┆ component: Points2D:colors        ┆ component: Points2D:positions                       │
│                     ┆                              ┆ component_type: Color             ┆ component_type: Position2D                          │
│                     ┆                              ┆ entity_path: /points              ┆ entity_path: /points                                │
│                     ┆                              ┆ kind: data                        ┆ kind: data                                          │
╞═════════════════════╪══════════════════════════════╪═══════════════════════════════════╪═════════════════════════════════════════════════════╡
│ simple_recording_id ┆ 2000-01-01T00:00:00          ┆ [4278190335, 16711935]            ┆ [[0.0, 1.0], [3.0, 4.0]]                            │
└─────────────────────┴──────────────────────────────┴───────────────────────────────────┴─────────────────────────────────────────────────────┘\
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
rerun_partition_id: [["simple_recording_id"]]
timeline: [[2000-01-01 00:00:00.000000000]]
/points:Points2D:colors: [[[4278190335,16711935]]]
/points:Points2D:positions: [[[[0,1],[3,4]]]]\
""")
