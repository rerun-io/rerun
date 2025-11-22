from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import rerun_draft as rr
from datafusion import col
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_catalog_basics(tmp_path: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        client.create_dataset("my_dataset")
        client.create_table("my_table", pa.schema([]), tmp_path.as_uri())

        df = client.entries()

        assert str(df.schema()) == inline_snapshot("""\
id: fixed_size_binary[16] not null
name: string not null
entry_kind: int32 not null  # Why is this int32 and not an enum? Poor py-arrow support?
created_at: timestamp[ns] not null
updated_at: timestamp[ns] not null
-- schema metadata --
sorbet:version: '0.1.1'\
""")

        assert str(
            df.drop("id", "created_at", "updated_at").filter(col("entry_kind") != 5).sort("name")
        ) == inline_snapshot(
            """\
┌─────────────────────────────┐
│ METADATA:                   │
│ * version: 0.1.1            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────┬────────────┐ │
│ │ name       ┆ entry_kind │ │
│ │ ---        ┆ ---        │ │
│ │ type: Utf8 ┆ type: i32  │ │
│ ╞════════════╪════════════╡ │
│ │ __entries  ┆ 3          │ │ # TODO(emilk): Can we remove __entries?
│ ├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ my_dataset ┆ 1          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ my_table   ┆ 3          │ │
│ └────────────┴────────────┘ │
└─────────────────────────────┘\
"""
        )
