from __future__ import annotations

import pprint
from typing import TYPE_CHECKING

import polars as pl
import pyarrow as pa
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_entries_to_polars(tmp_path: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        client.create_dataset("my_dataset")
        client.create_table("my_table", pa.schema([]), tmp_path.as_uri())

        # Test the new list-based entries() API
        entries = client.entries()
        assert sorted([e.name for e in entries]) == ["my_dataset", "my_table"]

        # Test get_table for raw __entries table access with polars conversion
        df = client.get_table(name="__entries").reader().to_polars()

        assert pprint.pformat(df.schema) == inline_snapshot(
            """\
Schema([('id', Binary),
        ('name', String),
        ('entry_kind', Int32),
        ('created_at', Datetime(time_unit='ns', time_zone=None)),
        ('updated_at', Datetime(time_unit='ns', time_zone=None))])\
"""
        )

        df = df.drop(["id", "created_at", "updated_at"]).filter(pl.col("entry_kind") != 5).sort("name")
        assert str(df) == inline_snapshot("""\
shape: (3, 2)
┌────────────┬────────────┐
│ name       ┆ entry_kind │
│ ---        ┆ ---        │
│ str        ┆ i32        │
╞════════════╪════════════╡
│ __entries  ┆ 3          │
│ my_dataset ┆ 1          │
│ my_table   ┆ 3          │
└────────────┴────────────┘\
""")


def test_table_to_polars(tmp_path: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()
        table = client.create_table(
            "my_table",
            pa.schema([pa.field("int16", pa.int16()), pa.field("string_list", pa.list_(pa.string()))]),
            tmp_path.as_uri(),
        )
        table.append(int16=[12], string_list=[["a", "b", "c"]])

        df = client.get_table(name="my_table").reader().to_polars()

        assert str(df) == inline_snapshot("""\
shape: (1, 2)
┌───────┬─────────────────┐
│ int16 ┆ string_list     │
│ ---   ┆ ---             │
│ i16   ┆ list[str]       │
╞═══════╪═════════════════╡
│ 12    ┆ ["a", "b", "c"] │
└───────┴─────────────────┘\
""")


def test_segment_table_to_polars(simple_dataset_prefix: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("my_dataset")
        ds.register_prefix(simple_dataset_prefix.as_uri())

        df = ds.segment_table().to_polars()

        assert pprint.pformat(df.schema) == inline_snapshot("""\
Schema([('rerun_segment_id', String),
        ('rerun_layer_names', List(String)),
        ('rerun_storage_urls', List(String)),
        ('rerun_last_updated_at', Datetime(time_unit='ns', time_zone=None)),
        ('rerun_num_chunks', UInt64),
        ('rerun_size_bytes', UInt64),
        ('timeline:end', Datetime(time_unit='ns', time_zone=None)),
        ('timeline:start', Datetime(time_unit='ns', time_zone=None))])\
""")

        df = df.drop(["rerun_storage_urls", "rerun_last_updated_at"]).sort("rerun_segment_id")
        assert str(df) == inline_snapshot("""\
shape: (3, 6)
┌────────────────┬────────────────┬────────────────┬───────────────┬───────────────┬───────────────┐
│ rerun_segment_ ┆ rerun_layer_na ┆ rerun_num_chun ┆ rerun_size_by ┆ timeline:end  ┆ timeline:star │
│ id             ┆ mes            ┆ ks             ┆ tes           ┆ ---           ┆ t             │
│ ---            ┆ ---            ┆ ---            ┆ ---           ┆ datetime[ns]  ┆ ---           │
│ str            ┆ list[str]      ┆ u64            ┆ u64           ┆               ┆ datetime[ns]  │
╞════════════════╪════════════════╪════════════════╪═══════════════╪═══════════════╪═══════════════╡
│ simple_recordi ┆ ["base"]       ┆ 2              ┆ 2092          ┆ 2000-01-01    ┆ 2000-01-01    │
│ ng_0           ┆                ┆                ┆               ┆ 00:00:00      ┆ 00:00:00      │
│ simple_recordi ┆ ["base"]       ┆ 2              ┆ 2092          ┆ 2000-01-01    ┆ 2000-01-01    │
│ ng_1           ┆                ┆                ┆               ┆ 00:00:01      ┆ 00:00:01      │
│ simple_recordi ┆ ["base"]       ┆ 2              ┆ 2092          ┆ 2000-01-01    ┆ 2000-01-01    │
│ ng_2           ┆                ┆                ┆               ┆ 00:00:02      ┆ 00:00:02      │
└────────────────┴────────────────┴────────────────┴───────────────┴───────────────┴───────────────┘\
""")


def test_dataframe_query_to_polars(simple_dataset_prefix: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("my_dataset")
        ds.register_prefix(simple_dataset_prefix.as_uri())

        # Create a view filtered to specific segments
        view = ds.filter_segments(["simple_recording_0", "simple_recording_2"])

        df = view.reader(index="timeline").to_polars()

        assert pprint.pformat(df.schema) == inline_snapshot("""\
Schema([('rerun_segment_id', String),
        ('timeline', Datetime(time_unit='ns', time_zone=None)),
        ('/points:Points2D:colors', List(UInt32)),
        ('/points:Points2D:positions', List(Array(Float32, shape=(2,))))])\
""")

        df = df.sort("rerun_segment_id")
        assert str(df) == inline_snapshot("""\
shape: (2, 4)
┌────────────────────┬─────────────────────┬─────────────────────────┬────────────────────────────┐
│ rerun_segment_id   ┆ timeline            ┆ /points:Points2D:colors ┆ /points:Points2D:positions │
│ ---                ┆ ---                 ┆ ---                     ┆ ---                        │
│ str                ┆ datetime[ns]        ┆ list[u32]               ┆ list[array[f32, 2]]        │
╞════════════════════╪═════════════════════╪═════════════════════════╪════════════════════════════╡
│ simple_recording_0 ┆ 2000-01-01 00:00:00 ┆ [4278190335, 16711935]  ┆ [[0.0, 1.0], [3.0, 4.0]]   │
│ simple_recording_2 ┆ 2000-01-01 00:00:02 ┆ [4278190847, 16712447]  ┆ [[2.0, 3.0], [5.0, 6.0]]   │
└────────────────────┴─────────────────────┴─────────────────────────┴────────────────────────────┘\
""")
