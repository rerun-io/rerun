from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from rerun.catalog import EntryKind
from rerun.server import Server

if TYPE_CHECKING:
    import pathlib

    from .conftest import PrefilledCatalog


def test_query_lance_table(prefilled_catalog: PrefilledCatalog) -> None:
    expected_table_name = "simple_datatypes"
    entries_table_name = "__entries"

    client = prefilled_catalog.client
    assert expected_table_name in client.table_names()
    assert entries_table_name in client.table_names()

    entries = client.table_entries()
    assert len(entries) == 4

    tables = client.tables()
    assert pa.Table.from_batches(tables.collect()).num_rows == 4

    client.get_table(name=expected_table_name)
    assert pa.Table.from_batches(tables.collect()).num_rows > 0

    entry = client.get_table_entry(name=expected_table_name)
    assert entry.name == expected_table_name
    assert entry.kind == EntryKind.TABLE


# TODO(#11852): fix this
@pytest.mark.skip(reason="This currently fails because of #11852")
def test_datafusion_catalog_get_tables(prefilled_catalog: PrefilledCatalog) -> None:
    ctx = prefilled_catalog.client.ctx

    # Verify we have the catalog provider and schema provider
    catalog_provider = ctx.catalog("datafusion")
    assert catalog_provider is not None

    schema_provider = catalog_provider.schema("public")
    assert schema_provider is not None

    # Note: as of DataFusion 50.0.0 this is not a DataFrame
    # but rather a python object that describes the table.
    table = schema_provider.table("simple_datatypes")
    assert table is not None

    schema_provider = catalog_provider.schema("second_schema")
    assert schema_provider.table("second_table") is not None

    catalog_provider = ctx.catalog("alternate_catalog")
    schema_provider = catalog_provider.schema("third_schema")
    assert schema_provider.table("third_table") is not None

    # Get by table name since it should be in the default catalog/schema
    df = ctx.table("simple_datatypes")
    rb = pa.Table.from_batches(df.collect())
    assert rb.num_rows > 0

    # Get table by fully qualified name
    df = ctx.table("datafusion.public.simple_datatypes")
    rb = pa.Table.from_batches(df.collect())
    assert rb.num_rows > 0

    # Verify SQL parsing for catalog provider works as expected
    df = ctx.sql("SELECT * FROM simple_datatypes")
    rb = pa.Table.from_batches(df.collect())
    assert rb.num_rows > 0

    df = ctx.sql("SELECT * FROM datafusion.public.simple_datatypes")
    rb = pa.Table.from_batches(df.collect())
    assert rb.num_rows > 0


# TODO(#11852): this demonstrates a working version of the previous test, to be removed once fixed
def test_datafusion_catalog_get_tables_patched(table_filepath: pathlib.Path) -> None:
    with Server(
        tables={
            "simple_datatypes": table_filepath,
            "second_schema.second_table": table_filepath,
            "alternate_catalog.third_schema.third_table": table_filepath,
        },
    ) as server:
        ctx = server.client().ctx

        # Verify we have the catalog provider and schema provider
        catalog_provider = ctx.catalog("datafusion")
        assert catalog_provider is not None

        schema_provider = catalog_provider.schema("public")
        assert schema_provider is not None

        # Note: as of DataFusion 50.0.0 this is not a DataFrame
        # but rather a python object that describes the table.
        table = schema_provider.table("simple_datatypes")
        assert table is not None

        schema_provider = catalog_provider.schema("second_schema")
        assert schema_provider.table("second_table") is not None

        catalog_provider = ctx.catalog("alternate_catalog")
        schema_provider = catalog_provider.schema("third_schema")
        assert schema_provider.table("third_table") is not None

        # Get by table name since it should be in the default catalog/schema
        df = ctx.table("simple_datatypes")
        rb = pa.Table.from_batches(df.collect())
        assert rb.num_rows > 0

        # Get table by fully qualified name
        df = ctx.table("datafusion.public.simple_datatypes")
        rb = pa.Table.from_batches(df.collect())
        assert rb.num_rows > 0

        # Verify SQL parsing for catalog provider works as expected
        df = ctx.sql("SELECT * FROM simple_datatypes")
        rb = pa.Table.from_batches(df.collect())
        assert rb.num_rows > 0

        df = ctx.sql("SELECT * FROM datafusion.public.simple_datatypes")
        rb = pa.Table.from_batches(df.collect())
        assert rb.num_rows > 0
