from __future__ import annotations

from typing import TYPE_CHECKING

import datafusion
import pyarrow as pa
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    import pytest


def test_table_api(tmp_path_factory: pytest.TempPathFactory) -> None:
    with rr.server.Server() as server:
        client = server.client()

        tmp_path = tmp_path_factory.mktemp("my_table")

        table = client.create_table_entry(
            "my_table",
            pa.schema([
                ("rerun_segment_id", pa.string()),
                ("operator", pa.string()),
            ]),
            tmp_path.as_uri(),
        )

        assert isinstance(table.df(), datafusion.DataFrame)

        assert str(table.df().schema()) == inline_snapshot("""\
rerun_segment_id: string
operator: string
-- schema metadata --
sorbet:version: '0.1.1'\
""")

        assert str(table.df().collect()) == inline_snapshot("[]")

        client.append_to_table(
            "my_table",
            rerun_segment_id=["segment_001", "segment_002", "segment_003"],
            operator=["alice", "bob", "carol"],
        )

        assert str(table.df().select("rerun_segment_id", "operator")) == inline_snapshot("""\
┌─────────────────────┬─────────────────────┐
│ rerun_segment_id    ┆ operator            │
│ ---                 ┆ ---                 │
│ type: nullable Utf8 ┆ type: nullable Utf8 │
╞═════════════════════╪═════════════════════╡
│ segment_001         ┆ alice               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ segment_002         ┆ bob                 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ segment_003         ┆ carol               │
└─────────────────────┴─────────────────────┘\
""")

        assert str(table.df()) == str(client.ctx.table("my_table"))
