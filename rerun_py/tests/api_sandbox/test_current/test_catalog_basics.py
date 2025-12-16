from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import rerun as rr
from datafusion import col
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_catalog_basics(tmp_path: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        client.create_dataset("my_dataset")
        client.create_table("my_table", pa.schema([]), tmp_path.as_uri())

        # Test the new list-based APIs
        datasets = client.datasets()
        assert [d.name for d in datasets] == ["my_dataset"]

        tables = client.tables()
        assert [t.name for t in tables] == ["my_table"]

        entries = client.entries()
        assert sorted([e.name for e in entries]) == ["my_dataset", "my_table"]

        # Test include_hidden parameter
        tables_with_hidden = client.tables(include_hidden=True)
        assert sorted([t.name for t in tables_with_hidden]) == ["__entries", "my_table"]

        # Test the underlying __entries table via get_table
        df = client.get_table(name="__entries").reader()

        assert str(df.schema()) == inline_snapshot("""\
id: fixed_size_binary[16] not null
name: string not null
entry_kind: int32 not null
created_at: timestamp[ns] not null
updated_at: timestamp[ns] not null
-- schema metadata --
sorbet:version: '0.1.2'\
""")

        assert str(
            df.drop("id", "created_at", "updated_at").filter(col("entry_kind") != 5).sort("name")
        ) == inline_snapshot(
            """\
┌─────────────────────────────┐
│ METADATA:                   │
│ * version: 0.1.2            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────┬────────────┐ │
│ │ name       ┆ entry_kind │ │
│ │ ---        ┆ ---        │ │
│ │ type: Utf8 ┆ type: i32  │ │
│ ╞════════════╪════════════╡ │
│ │ __entries  ┆ 3          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ my_dataset ┆ 1          │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ my_table   ┆ 3          │ │
│ └────────────┴────────────┘ │
└─────────────────────────────┘\
"""
        )
