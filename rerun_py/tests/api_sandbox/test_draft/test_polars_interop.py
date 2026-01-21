from __future__ import annotations

import pprint
from typing import TYPE_CHECKING

import pyarrow as pa
import rerun_draft as rr
from datafusion import col, lit
from datafusion.functions import in_list
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


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
│ simple_recordi ┆ ["base"]       ┆ 2              ┆ 2656          ┆ 2000-01-01    ┆ 2000-01-01    │
│ ng_0           ┆                ┆                ┆               ┆ 00:00:00      ┆ 00:00:00      │
│ simple_recordi ┆ ["base"]       ┆ 2              ┆ 2656          ┆ 2000-01-01    ┆ 2000-01-01    │
│ ng_1           ┆                ┆                ┆               ┆ 00:00:01      ┆ 00:00:01      │
│ simple_recordi ┆ ["base"]       ┆ 2              ┆ 2656          ┆ 2000-01-01    ┆ 2000-01-01    │
│ ng_2           ┆                ┆                ┆               ┆ 00:00:02      ┆ 00:00:02      │
└────────────────┴────────────────┴────────────────┴───────────────┴───────────────┴───────────────┘\
""")


def test_dataframe_query_to_polars(simple_dataset_prefix: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("my_dataset")
        ds.register_prefix(simple_dataset_prefix.as_uri())

        df = (
            ds.reader(index="timeline")
            # All former view-level filtering happens now in datafusion and is (hopefully) pushed back
            .filter(in_list(col("rerun_segment_id"), [lit("simple_recording_0"), lit("simple_recording_2")]))
            .to_polars()
        )

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
