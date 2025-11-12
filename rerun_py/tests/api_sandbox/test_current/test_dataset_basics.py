from __future__ import annotations

from typing import TYPE_CHECKING

import rerun as rr
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_dataset_basics(simple_dataset_prefix: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        ds = client.create_dataset("basic_dataset")

        ds.register_prefix(simple_dataset_prefix.as_uri())

        partition_df = ds.partition_table().df()

        assert str(partition_df.schema()) == inline_snapshot("""\
rerun_partition_id: string not null
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
            partition_df.drop("rerun_storage_urls", "rerun_last_updated_at").sort("rerun_partition_id")
        ) == inline_snapshot("""\
┌────────────────────┬───────────────────┬──────────────────┬──────────────────┐
│ rerun_partition_id ┆ rerun_layer_names ┆ rerun_num_chunks ┆ rerun_size_bytes │
│ ---                ┆ ---               ┆ ---              ┆ ---              │
│ type: Utf8         ┆ type: List[Utf8]  ┆ type: u64        ┆ type: u64        │
╞════════════════════╪═══════════════════╪══════════════════╪══════════════════╡
│ simple_recording_0 ┆ [base]            ┆ 2                ┆ 1392             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ simple_recording_1 ┆ [base]            ┆ 2                ┆ 1392             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ simple_recording_2 ┆ [base]            ┆ 2                ┆ 1392             │
└────────────────────┴───────────────────┴──────────────────┴──────────────────┘\
""")
