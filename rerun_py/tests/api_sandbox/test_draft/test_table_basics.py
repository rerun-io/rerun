from __future__ import annotations

import pyarrow as pa
import rerun_draft as rr
from inline_snapshot import snapshot as inline_snapshot
from rerun.catalog import TableInsertMode


def test_create_table_and_append() -> None:
    """Create a table and append data using Python values."""
    with rr.server.Server() as server:
        client = server.client()

        # Create a table with a schema
        schema = pa.schema([
            pa.field("id", pa.int32()),
            pa.field("value", pa.float64()),
            pa.field("enabled", pa.bool_()),
        ])

        table = client.create_table("my_table", schema)

        # Append single row with scalar values
        table.append(id=1, value=10.5, enabled=True)

        # Append multiple rows using lists
        table.append(id=[2, 3, 4], value=[20.3, 15.7, 30.2], enabled=[False, True, False])

        # Read the table back
        df = table.reader()

        assert str(df.sort("id")) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────┐
│ METADATA:                                                         │
│ * version: 0.1.2                                                  │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────────────┬────────────────────┬─────────────────────┐ │
│ │ id                 ┆ value              ┆ enabled             │ │
│ │ ---                ┆ ---                ┆ ---                 │ │
│ │ type: nullable i32 ┆ type: nullable f64 ┆ type: nullable bool │ │
│ ╞════════════════════╪════════════════════╪═════════════════════╡ │
│ │ 1                  ┆ 10.5               ┆ true                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 2                  ┆ 20.3               ┆ false               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3                  ┆ 15.7               ┆ true                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 4                  ┆ 30.2               ┆ false               │ │
│ └────────────────────┴────────────────────┴─────────────────────┘ │
└───────────────────────────────────────────────────────────────────┘\
""")


def test_write_table_with_record_batches() -> None:
    """Write PyArrow RecordBatches to a table."""
    with rr.server.Server() as server:
        client = server.client()

        schema = pa.schema([
            pa.field("id", pa.int32()),
            pa.field("enabled", pa.bool_()),
            pa.field("score", pa.float64()),
        ])

        client.create_table("scores_table", schema)

        # Create record batches
        batch1 = pa.RecordBatch.from_pydict(
            {"id": [1, 2, 3], "enabled": [True, False, True], "score": [95.5, 87.3, 91.2]}, schema=schema
        )

        batch2 = pa.RecordBatch.from_pydict(
            {"id": [4, 5, 6], "enabled": [True, True, False], "score": [88.7, 93.1, 85.4]}, schema=schema
        )

        # Append batches to table
        client.write_table("scores_table", [batch1, batch2], TableInsertMode.APPEND)

        # Query the table
        df = client.get_table(name="scores_table").reader()

        assert str(df.sort("id")) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────┐
│ METADATA:                                                         │
│ * version: 0.1.2                                                  │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌────────────────────┬─────────────────────┬────────────────────┐ │
│ │ id                 ┆ enabled             ┆ score              │ │
│ │ ---                ┆ ---                 ┆ ---                │ │
│ │ type: nullable i32 ┆ type: nullable bool ┆ type: nullable f64 │ │
│ ╞════════════════════╪═════════════════════╪════════════════════╡ │
│ │ 1                  ┆ true                ┆ 95.5               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 2                  ┆ false               ┆ 87.3               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3                  ┆ true                ┆ 91.2               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 4                  ┆ true                ┆ 88.7               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 5                  ┆ true                ┆ 93.1               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 6                  ┆ false               ┆ 85.4               │ │
│ └────────────────────┴─────────────────────┴────────────────────┘ │
└───────────────────────────────────────────────────────────────────┘\
""")


def test_table_overwrite_mode() -> None:
    """Demonstrate APPEND vs OVERWRITE modes when writing to tables."""
    with rr.server.Server() as server:
        client = server.client()

        schema = pa.schema([pa.field("id", pa.int32()), pa.field("category", pa.string())])

        client.create_table("data_table", schema)

        # Initial data
        batch1 = pa.RecordBatch.from_pydict({"id": [1, 2, 3], "category": ["A", "B", "C"]}, schema=schema)

        client.write_table("data_table", batch1, TableInsertMode.APPEND)

        df_after_append = client.get_table(name="data_table").reader()
        assert str(df_after_append.sort("id")) == inline_snapshot("""\
┌────────────────────┬─────────────────────┐
│ id                 ┆ category            │
│ ---                ┆ ---                 │
│ type: nullable i32 ┆ type: nullable Utf8 │
╞════════════════════╪═════════════════════╡
│ 1                  ┆ A                   │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 2                  ┆ B                   │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 3                  ┆ C                   │
└────────────────────┴─────────────────────┘\
""")

        # Overwrite with new data
        batch2 = pa.RecordBatch.from_pydict({"id": [10, 20], "category": ["X", "Y"]}, schema=schema)

        client.write_table("data_table", batch2, TableInsertMode.OVERWRITE)

        df_after_overwrite = client.get_table(name="data_table").reader()
        assert str(df_after_overwrite.sort("id")) == inline_snapshot("""\
┌────────────────────┬─────────────────────┐
│ id                 ┆ category            │
│ ---                ┆ ---                 │
│ type: nullable i32 ┆ type: nullable Utf8 │
╞════════════════════╪═════════════════════╡
│ 10                 ┆ X                   │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 20                 ┆ Y                   │
└────────────────────┴─────────────────────┘\
""")


def test_read_table_entry() -> None:
    """Demonstrate three ways to read table data: get_table, get_table_entry, and via DataFusion context."""
    with rr.server.Server() as server:
        client = server.client()

        schema = pa.schema([pa.field("product_id", pa.int32()), pa.field("price", pa.float64())])

        client.create_table("products", schema)

        # Add some data
        client.append_to_table("products", product_id=[101, 102, 103], price=[29.99, 49.99, 19.99])

        # Method 1: get_table - returns a TableEntry and call reader() to get DataFrame
        table1 = client.get_table(name="products")
        df1 = table1.reader()
        assert str(df1.sort("product_id")) == inline_snapshot("""\
┌────────────────────┬────────────────────┐
│ product_id         ┆ price              │
│ ---                ┆ ---                │
│ type: nullable i32 ┆ type: nullable f64 │
╞════════════════════╪════════════════════╡
│ 101                ┆ 29.99              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 102                ┆ 49.99              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 103                ┆ 19.99              │
└────────────────────┴────────────────────┘\
""")

        # Method 2: Directly call reader on a table entry stored before
        df2 = table1.reader()
        assert str(df2.sort("product_id")) == inline_snapshot("""\
┌────────────────────┬────────────────────┐
│ product_id         ┆ price              │
│ ---                ┆ ---                │
│ type: nullable i32 ┆ type: nullable f64 │
╞════════════════════╪════════════════════╡
│ 101                ┆ 29.99              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 102                ┆ 49.99              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 103                ┆ 19.99              │
└────────────────────┴────────────────────┘\
""")

        # Method 3: via DataFusion SessionContext - useful for SQL queries
        ctx = client.ctx
        df3 = ctx.table("products")
        assert str(df3.sort("product_id")) == inline_snapshot("""\
┌────────────────────┬────────────────────┐
│ product_id         ┆ price              │
│ ---                ┆ ---                │
│ type: nullable i32 ┆ type: nullable f64 │
╞════════════════════╪════════════════════╡
│ 101                ┆ 29.99              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 102                ┆ 49.99              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 103                ┆ 19.99              │
└────────────────────┴────────────────────┘\
""")
