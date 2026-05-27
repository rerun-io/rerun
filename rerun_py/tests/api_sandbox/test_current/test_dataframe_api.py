from __future__ import annotations

import datetime
from typing import TYPE_CHECKING

import numpy as np
import rerun as rr
from datafusion import col, lit
from datafusion.functions import in_list
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_dataframe_api_filter_segment_id(simple_dataset_prefix: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_dataset_prefix}) as server:
        client = server.client()
        ds = client.get_dataset(name="ds")

        # Create a view filtered to specific segments
        view = ds.filter_segments(["simple_recording_0", "simple_recording_2"])

        # Get dataframe from the filtered view
        df = view.reader(index="timeline").sort("rerun_segment_id")

        assert str(df) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                     │
│ * version: 0.1.3                                                                                                              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬──────────────────────┬────────────────────────────┬─────────────────────────────────────────────────┐ │
│ │ rerun_segment_id    ┆ timeline             ┆ /points:Points2D:colors    ┆ /points:Points2D:positions                      │ │
│ │ ---                 ┆ ---                  ┆ ---                        ┆ ---                                             │ │
│ │ type: non-null Utf8 ┆ type: Timestamp(ns)  ┆ type: List(UInt32)         ┆ type: List(FixedSizeList(2 x non-null Float32)) │ │
│ │                     ┆ index_name: timeline ┆ archetype: Points2D        ┆ archetype: Points2D                             │ │
│ │                     ┆ kind: index          ┆ component: Points2D:colors ┆ component: Points2D:positions                   │ │
│ │                     ┆                      ┆ component_type: Color      ┆ component_type: Position2D                      │ │
│ │                     ┆                      ┆ entity_path: /points       ┆ entity_path: /points                            │ │
│ │                     ┆                      ┆ kind: data                 ┆ kind: data                                      │ │
│ ╞═════════════════════╪══════════════════════╪════════════════════════════╪═════════════════════════════════════════════════╡ │
│ │ simple_recording_0  ┆ 2000-01-01T00:00:00  ┆ [4278190335, 16711935]     ┆ [[0.0, 1.0], [3.0, 4.0]]                        │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_2  ┆ 2000-01-01T00:00:02  ┆ [4278190847, 16712447]     ┆ [[2.0, 3.0], [5.0, 6.0]]                        │ │
│ └─────────────────────┴──────────────────────┴────────────────────────────┴─────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
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


# An unknown segment ID must succeed with zero rows along every entry point —
# `filter_segments`, `using_index_values`, and DataFusion `WHERE rerun_segment_id`
# filter pushdown. All three share the same `QueryDatasetRequest.segment_ids`
# field on the wire, so the server cannot tell them apart; behavior is uniform
# by construction. Validating IDs would cost a server roundtrip and turn SQL
# filters into hand-grenades — callers needing typo detection should validate
# client-side.


def test_dataframe_api_filter_segments_unknown(simple_dataset_prefix: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_dataset_prefix}) as server:
        client = server.client()
        ds = client.get_dataset(name="ds")

        view = ds.filter_segments(["does_not_exist"])
        table = view.reader(index="timeline").to_arrow_table()

        assert table.num_rows == 0


def test_dataframe_api_using_index_values_unknown(simple_dataset_prefix: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_dataset_prefix}) as server:
        client = server.client()
        ds = client.get_dataset(name="ds")

        table = ds.reader(
            index="timeline",
            using_index_values={
                "does_not_exist": np.array(
                    [datetime.datetime(2000, 1, 1, 0, 0, 0)],
                    dtype="datetime64[ns]",
                ),
            },
        ).to_arrow_table()

        assert table.num_rows == 0


def test_dataframe_api_filter_unknown_segment_id_pushdown(simple_dataset_prefix: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_dataset_prefix}) as server:
        client = server.client()
        ds = client.get_dataset(name="ds")

        table = ds.reader(index="timeline").filter(col("rerun_segment_id") == "does_not_exist").to_arrow_table()

        assert table.num_rows == 0


# Heterogeneous variants: one known + one unknown segment id. The unknown one
# must contribute zero rows; the result is exactly what the known segment
# would yield on its own. Same three entry points.


def test_dataframe_api_filter_segments_mixed_known_unknown(simple_dataset_prefix: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_dataset_prefix}) as server:
        client = server.client()
        ds = client.get_dataset(name="ds")

        table = ds.filter_segments(["simple_recording_0", "does_not_exist"]).reader(index="timeline").to_arrow_table()

        assert table.column("rerun_segment_id").to_pylist() == ["simple_recording_0"]


def test_dataframe_api_using_index_values_mixed_known_unknown(simple_dataset_prefix: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_dataset_prefix}) as server:
        client = server.client()
        ds = client.get_dataset(name="ds")

        table = ds.reader(
            index="timeline",
            using_index_values={
                "simple_recording_0": np.array(
                    [datetime.datetime(2000, 1, 1, 0, 0, 0)],
                    dtype="datetime64[ns]",
                ),
                "does_not_exist": np.array(
                    [datetime.datetime(2000, 1, 1, 0, 0, 0)],
                    dtype="datetime64[ns]",
                ),
            },
        ).to_arrow_table()

        assert table.column("rerun_segment_id").to_pylist() == ["simple_recording_0"]


def test_dataframe_api_filter_mixed_segment_id_pushdown(simple_dataset_prefix: Path) -> None:
    with rr.server.Server(datasets={"ds": simple_dataset_prefix}) as server:
        client = server.client()
        ds = client.get_dataset(name="ds")

        table = (
            ds
            .reader(index="timeline")
            .filter(in_list(col("rerun_segment_id"), [lit("simple_recording_0"), lit("does_not_exist")]))
            .to_arrow_table()
        )

        assert table.column("rerun_segment_id").to_pylist() == ["simple_recording_0"]
