from __future__ import annotations

import datafusion
import pyarrow as pa
import rerun_draft as rr
from inline_snapshot import snapshot as inline_snapshot


def test_table_api() -> None:
    with rr.server.Server() as server:
        client = server.client()

        table = client.create_table(
            "my_table",
            pa.schema([
                ("rerun_segment_id", pa.string()),
                ("operator", pa.string()),
            ]),
        )

        assert str(table.arrow_schema()) == inline_snapshot("""\
rerun_segment_id: string
operator: string
-- schema metadata --
sorbet:version: '0.1.2'\
""")
        reader = table.reader()

        assert isinstance(reader, datafusion.DataFrame)

        assert str(table.reader().collect()) == inline_snapshot("[]")

        table.append(
            rerun_segment_id=["segment_001", "segment_002"],
            operator=["alice", "bob"],
        )

        assert str(reader.sort("rerun_segment_id")) == inline_snapshot("""\
┌─────────────────────┬─────────────────────┐
│ rerun_segment_id    ┆ operator            │
│ ---                 ┆ ---                 │
│ type: nullable Utf8 ┆ type: nullable Utf8 │
╞═════════════════════╪═════════════════════╡
│ segment_001         ┆ alice               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ segment_002         ┆ bob                 │
└─────────────────────┴─────────────────────┘\
""")

        table.append(
            rerun_segment_id="segment_003",
            operator="carol",
        )

        assert str(reader.sort("rerun_segment_id")) == inline_snapshot("""\
┌───────────────────────────────────────────────┐
│ METADATA:                                     │
│ * version: 0.1.2                              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬─────────────────────┐ │
│ │ rerun_segment_id    ┆ operator            │ │
│ │ ---                 ┆ ---                 │ │
│ │ type: nullable Utf8 ┆ type: nullable Utf8 │ │
│ ╞═════════════════════╪═════════════════════╡ │
│ │ segment_001         ┆ alice               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ segment_002         ┆ bob                 │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ segment_003         ┆ carol               │ │
│ └─────────────────────┴─────────────────────┘ │
└───────────────────────────────────────────────┘\
""")

        assert str(reader) == str(client.ctx.table("my_table"))

        batches = [
            pa.RecordBatch.from_pydict({"rerun_segment_id": ["segment_004"], "operator": ["dan"]}),
            pa.RecordBatch.from_pydict({"rerun_segment_id": ["segment_005"], "operator": ["erin"]}),
        ]

        table.append(batches)
