from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
from datafusion import DataFrameWriteOptions, InsertOp, SessionContext, col

from rerun.catalog import TableInsertMode

if TYPE_CHECKING:
    from .conftest import ServerInstance


def test_datafusion_write_table(server_instance: ServerInstance) -> None:
    table_name = "simple_datatypes"
    ctx: SessionContext = server_instance.client.ctx

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


def test_client_write_table(server_instance: ServerInstance) -> None:
    table_name = "simple_datatypes"
    ctx: SessionContext = server_instance.client.ctx

    df_prior = ctx.table(table_name)
    original_count = df_prior.count()

    schema = pa.schema([("id", pa.int32()), ("bool_col", pa.bool_()), ("double_col", pa.float64())])

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
    server_instance.client.write_table(table_name, reader, TableInsertMode.APPEND)
    final_count = ctx.table(table_name).count()
    assert final_count == original_count + 9

    # Test with a list of list of record batches, like a collect() will give you
    server_instance.client.write_table(table_name, [[batch1, batch2], [batch3]], TableInsertMode.APPEND)
    final_count = ctx.table(table_name).count()
    assert final_count == original_count + 18

    # Test with a list of record batches
    server_instance.client.write_table(table_name, [batch1, batch2, batch3], TableInsertMode.APPEND)
    final_count = ctx.table(table_name).count()
    assert final_count == original_count + 27

    # Test with a single record batch
    server_instance.client.write_table(table_name, batch1, TableInsertMode.APPEND)
    final_count = ctx.table(table_name).count()
    assert final_count == original_count + 30

    # Test overwrite method
    server_instance.client.write_table(table_name, batch1, TableInsertMode.OVERWRITE)
    final_count = ctx.table(table_name).count()
    assert final_count == 3


def test_client_append_to_table(server_instance: ServerInstance) -> None:
    table_name = "simple_datatypes"
    ctx: SessionContext = server_instance.client.ctx

    original_rows = ctx.table(table_name).count()

    server_instance.client.append_to_table(table_name, id=3, bool_col=True, double_col=2.0)
    assert ctx.table(table_name).count() == original_rows + 1

    server_instance.client.append_to_table(
        table_name, id=[3, 4, 5], bool_col=[False, True, None], double_col=[2.0, None, 1.0]
    )
    assert ctx.table(table_name).count() == original_rows + 4
