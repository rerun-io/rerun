from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import rerun_draft as rr
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_dataset_basics(simple_dataset_prefix: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        ds = client.create_dataset("basic_dataset")

        ds.register_prefix(simple_dataset_prefix.as_uri())

        segment_df = ds.segment_table()

        assert str(segment_df.schema()) == inline_snapshot("""\
rerun_segment_id: string not null
rerun_layer_names: list<rerun_layer_names: string not null> not null
  child 0, rerun_layer_names: string not null
rerun_storage_urls: list<rerun_storage_urls: string not null> not null
  child 0, rerun_storage_urls: string not null
rerun_last_updated_at: timestamp[ns] not null
rerun_num_chunks: uint64 not null
rerun_size_bytes: uint64 not null
-- schema metadata --
sorbet:version: '0.1.1'\
""")

        assert str(
            segment_df.drop("rerun_storage_urls", "rerun_last_updated_at").sort("rerun_segment_id")
        ) == inline_snapshot("""\
┌──────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                        │
│ * version: 0.1.1                                                                 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────────────┬───────────────────┬──────────────────┬──────────────────┐ │
│ │ rerun_segment_id   ┆ rerun_layer_names ┆ rerun_num_chunks ┆ rerun_size_bytes │ │
│ │ ---                ┆ ---               ┆ ---              ┆ ---              │ │
│ │ type: Utf8         ┆ type: List[Utf8]  ┆ type: u64        ┆ type: u64        │ │
│ ╞════════════════════╪═══════════════════╪══════════════════╪══════════════════╡ │
│ │ simple_recording_0 ┆ [base]            ┆ 2                ┆ 1392             │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_1 ┆ [base]            ┆ 2                ┆ 1392             │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_2 ┆ [base]            ┆ 2                ┆ 1392             │ │
│ └────────────────────┴───────────────────┴──────────────────┴──────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_dataset_metadata(simple_dataset_prefix: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        ds = client.create_dataset("basic_dataset")
        ds.register_prefix(simple_dataset_prefix.as_uri())

        # TODO(jleibs): Consider attaching this metadata table directly to the dataset
        # and automatically joining it by default
        meta = client.create_table(
            "basic_dataset_metadata",
            pa.schema([
                ("rerun_segment_id", pa.string()),
                ("success", pa.bool_()),
            ]),
        )

        meta.append(
            rerun_segment_id=["simple_recording_0", "simple_recording_2"],
            success=[True, False],
        )

        joined = ds.segment_table(join_meta=meta)

        assert str(
            joined.drop("rerun_storage_urls", "rerun_last_updated_at").sort("rerun_segment_id")
        ) == inline_snapshot("""\
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                              │
│ * version: 0.1.1                                                                                       │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────────────┬───────────────────┬──────────────────┬──────────────────┬─────────────────────┐ │
│ │ rerun_segment_id   ┆ rerun_layer_names ┆ rerun_num_chunks ┆ rerun_size_bytes ┆ success             │ │
│ │ ---                ┆ ---               ┆ ---              ┆ ---              ┆ ---                 │ │
│ │ type: Utf8         ┆ type: List[Utf8]  ┆ type: u64        ┆ type: u64        ┆ type: nullable bool │ │
│ ╞════════════════════╪═══════════════════╪══════════════════╪══════════════════╪═════════════════════╡ │
│ │ simple_recording_0 ┆ [base]            ┆ 2                ┆ 1392             ┆ true                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_1 ┆ [base]            ┆ 2                ┆ 1392             ┆ null                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ simple_recording_2 ┆ [base]            ┆ 2                ┆ 1392             ┆ false               │ │
│ └────────────────────┴───────────────────┴──────────────────┴──────────────────┴─────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")
