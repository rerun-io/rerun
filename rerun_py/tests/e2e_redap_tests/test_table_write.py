from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from datafusion import DataFrameWriteOptions, InsertOp, SessionContext, col

if TYPE_CHECKING:
    import pathlib

    from syrupy import SnapshotAssertion

    from .conftest import EntryFactory


@pytest.mark.creates_table
def test_datafusion_write_table(entry_factory: EntryFactory, tmp_path: pathlib.Path) -> None:
    """Test DataFusion write operations (append/overwrite) on a table created from scratch."""
    base_name = "test_table"
    table_name = entry_factory.apply_prefix(base_name)
    ctx: SessionContext = entry_factory.client.ctx

    # Create a table from scratch
    schema = pa.schema([("id", pa.int32()), ("value", pa.float64())])
    table = entry_factory.create_table(base_name, schema, tmp_path.absolute().as_uri())

    # Write initial data
    initial_data = pa.RecordBatch.from_pydict(
        {"id": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10], "value": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]},
        schema=schema,
    )
    table.append(initial_data)

    df_prior = ctx.table(table_name)
    prior_count = df_prior.count()

    df_smaller = ctx.table(table_name).filter(col("id") < 5)
    smaller_count = df_smaller.count()

    # Verify append mode
    df_smaller.write_table(table_name)
    assert ctx.table(table_name).count() == prior_count + smaller_count

    # Verify overwrite mode
    df_smaller.write_table(table_name, write_options=DataFrameWriteOptions(insert_operation=InsertOp.OVERWRITE))
    assert ctx.table(table_name).count() == smaller_count


@pytest.mark.creates_table
def test_client_write_table(entry_factory: EntryFactory, tmp_path: pathlib.Path) -> None:
    """Test client write operations with various input formats on a table created from scratch."""
    base_name = "test_table"
    table_name = entry_factory.apply_prefix(base_name)
    ctx: SessionContext = entry_factory.client.ctx

    # Create a table from scratch
    schema = pa.schema([("id", pa.int32()), ("bool_col", pa.bool_()), ("double_col", pa.float64())])
    table = entry_factory.create_table(base_name, schema, tmp_path.absolute().as_uri())

    # No initial data, start with empty table
    original_count = 0

    batch1 = pa.RecordBatch.from_pydict(
        {"id": [1, 2, 3], "bool_col": [True, False, None], "double_col": [10.5, 20.3, 15.7]}, schema=schema
    )

    batch2 = pa.RecordBatch.from_pydict(
        {"id": [4, 5, 6], "bool_col": [True, None, False], "double_col": [30.2, 25.8, 18.9]}, schema=schema
    )

    batch3 = pa.RecordBatch.from_pydict(
        {"id": [7, 8, 9], "bool_col": [True, True, False], "double_col": [22.4, 28.1, 31.5]}, schema=schema
    )

    # Test with a record batch reader
    reader = pa.RecordBatchReader.from_batches(schema, [batch1, batch2, batch3])
    table.append(reader)
    final_count = ctx.table(table_name).count()
    assert final_count == original_count + 9

    # Test with a list of list of record batches, like a collect() will give you
    table.append([[batch1, batch2], [batch3]])
    final_count = ctx.table(table_name).count()
    assert final_count == original_count + 18

    # Test with a list of record batches
    table.append([batch1, batch2, batch3])
    final_count = ctx.table(table_name).count()
    assert final_count == original_count + 27

    # Test with a single record batch
    table.append(batch1)
    final_count = ctx.table(table_name).count()
    assert final_count == original_count + 30

    # Test overwrite method
    table.overwrite(batch1)
    final_count = ctx.table(table_name).count()
    assert final_count == 3


@pytest.mark.creates_table
def test_client_append_to_table(entry_factory: EntryFactory, tmp_path: pathlib.Path) -> None:
    """Test TableEntry.append() convenience method on a table created from scratch."""
    base_name = "test_table"
    table_name = entry_factory.apply_prefix(base_name)
    ctx: SessionContext = entry_factory.client.ctx

    # Create a table from scratch
    schema = pa.schema([("id", pa.int32()), ("bool_col", pa.bool_()), ("double_col", pa.float64())])
    table = entry_factory.create_table(base_name, schema, tmp_path.absolute().as_uri())

    # Start with empty table
    original_rows = 0

    table.append(id=3, bool_col=True, double_col=2.0)
    assert ctx.table(table_name).count() == original_rows + 1

    table.append(id=[3, 4, 5], bool_col=[False, True, None], double_col=[2.0, None, 1.0])
    assert ctx.table(table_name).count() == original_rows + 4


@pytest.mark.creates_table
def test_table_upsert(entry_factory: EntryFactory, tmp_path: pathlib.Path, snapshot: SnapshotAssertion) -> None:
    """Test TableEntry.upsert() method on a table with an index column."""
    base_name = "test_table"
    table_name = entry_factory.apply_prefix(base_name)
    ctx: SessionContext = entry_factory.client.ctx

    # Create a table with an index column (marked with rerun:is_table_index metadata)
    schema = pa.schema([
        pa.field("id", pa.int32(), metadata={"rerun:is_table_index": "true"}),
        pa.field("value", pa.string()),
    ])
    table = entry_factory.create_table(base_name, schema, tmp_path.absolute().as_uri())

    # Insert initial data
    initial_batch = pa.RecordBatch.from_pydict({"id": [1, 2, 3], "value": ["a", "b", "c"]}, schema=schema)
    table.append(initial_batch)
    assert ctx.table(table_name).count() == 3

    # Upsert: update existing rows (id=1, id=2) and insert new row (id=4)
    upsert_batch = pa.RecordBatch.from_pydict({"id": [1, 2, 4], "value": ["A", "B", "D"]}, schema=schema)
    table.upsert(upsert_batch)

    # Should still have 4 rows (3 original, minus 2 updated, plus 1 new = 4)
    assert ctx.table(table_name).count() == 4

    # Verify the values were updated
    result = ctx.table(table_name).sort(col("id")).collect()
    assert len(result) == 1
    batch = result[0]
    assert str(batch) == snapshot


@pytest.mark.local_only
def test_write_to_registered_table(entry_factory: EntryFactory, tmp_path: pathlib.Path) -> None:
    """
    Test writing to a pre-registered table (not created from scratch).

    This test is marked as local_only because:
    1. It needs to copy the table to avoid polluting the original
    2. Remote deployments can't access local file:// URIs for the copy
    """
    import shutil

    from .conftest import TABLE_FILEPATH

    # Copy table to temp directory to avoid polluting the original
    temp_table_path = tmp_path / "simple_datatypes_copy"
    shutil.copytree(TABLE_FILEPATH, temp_table_path)

    # Register the copied table
    base_name = "registered_table"
    table_name = entry_factory.apply_prefix(base_name)
    entry_factory.register_table(base_name, temp_table_path.as_uri())

    # Verify we can query the registered table
    ctx: SessionContext = entry_factory.client.ctx
    original_count = ctx.table(table_name).count()
    assert original_count > 0  # Should have some data

    # Write to it
    schema = pa.schema([("id", pa.int32()), ("bool_col", pa.bool_()), ("double_col", pa.float64())])
    batch = pa.RecordBatch.from_pydict({"id": [999], "bool_col": [True], "double_col": [99.9]}, schema=schema)
    table = entry_factory.client.get_table(name=table_name)
    table.append(batch)

    # Verify the write succeeded
    assert ctx.table(table_name).count() == original_count + 1
