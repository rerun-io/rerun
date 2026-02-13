from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import rerun_draft as rr
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_catalog_basics(tmp_path: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        client.create_dataset("my_dataset")
        client.create_table("my_table", pa.schema([]), tmp_path.as_uri())

        assert str(client.entries()) == inline_snapshot(
            "[Entry(EntryKind.DATASET, 'my_dataset'), Entry(EntryKind.TABLE, 'my_table')]"
        )

        assert str(client.datasets()) == inline_snapshot("[Entry(EntryKind.DATASET, 'my_dataset')]")

        assert str(client.tables()) == inline_snapshot("[Entry(EntryKind.TABLE, 'my_table')]")

        assert str(client.tables(include_hidden=True)) == inline_snapshot(
            "[Entry(EntryKind.TABLE, '__entries'), Entry(EntryKind.TABLE, 'my_table')]"
        )


def test_catalog_modify() -> None:
    with rr.server.Server() as server:
        client = server.client()

        table1 = client.create_table("table1", pa.schema([]))
        table2 = client.create_table("table2", pa.schema([]))

        assert str(client.tables()) == inline_snapshot(
            "[Entry(EntryKind.TABLE, 'table1'), Entry(EntryKind.TABLE, 'table2')]"
        )

        table1.set_name("table_one")

        assert str(client.tables()) == inline_snapshot(
            "[Entry(EntryKind.TABLE, 'table2'), Entry(EntryKind.TABLE, 'table_one')]"
        )

        table2.delete()

        assert str(client.tables()) == inline_snapshot("[Entry(EntryKind.TABLE, 'table_one')]")
