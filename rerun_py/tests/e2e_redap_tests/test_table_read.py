from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
from rerun.catalog import EntryKind

if TYPE_CHECKING:
    from .conftest import PrefilledCatalog


def test_query_lance_table(prefilled_catalog: PrefilledCatalog) -> None:
    table_name = "simple_datatypes"
    expected_table_name = prefilled_catalog.factory.apply_prefix(table_name)
    entries_table_name = "__entries"

    client = prefilled_catalog.client
    assert expected_table_name in client.table_names()
    assert entries_table_name in client.table_names(include_hidden=True)

    entries = client.tables()

    # Check that we have at least the expected tables (may have more on external servers).
    # 3 tables in `PrefilledCatalog`, or 4 accounting for `__entries`.
    assert len(entries) >= 3
    assert len(client.tables(include_hidden=True)) >= 4

    entry_names = [e.name for e in entries]
    assert expected_table_name in entry_names
    assert prefilled_catalog.factory.apply_prefix("second_schema.second_table") in entry_names
    assert prefilled_catalog.factory.apply_prefix("alternate_catalog.third_schema.third_table") in entry_names

    # Verify we can get and query the table
    entry = client.get_table(name=expected_table_name)
    entries_df = client.get_table(name="__entries").reader()
    assert pa.Table.from_batches(entries_df.collect()).num_rows > 0
    assert entry.name == expected_table_name
    assert entry.kind == EntryKind.TABLE


def test_datafusion_catalog_get_tables(prefilled_catalog: PrefilledCatalog) -> None:
    ctx = prefilled_catalog.client.ctx

    # Verify we have the catalog provider and schema provider
    catalog_provider = ctx.catalog("datafusion")
    assert catalog_provider is not None

    schema_provider = catalog_provider.schema("public")
    assert schema_provider is not None

    # Note: as of DataFusion 50.0.0 this is not a DataFrame
    # but rather a python object that describes the table.
    table = schema_provider.table(prefilled_catalog.factory.apply_prefix("simple_datatypes"))
    assert table is not None

    schema_provider = catalog_provider.schema("second_schema")
    assert schema_provider.table(prefilled_catalog.factory.apply_prefix("second_table")) is not None

    catalog_provider = ctx.catalog("alternate_catalog")
    schema_provider = catalog_provider.schema("third_schema")
    assert schema_provider.table(prefilled_catalog.factory.apply_prefix("third_table")) is not None

    # Get by table name since it should be in the default catalog/schema
    df = ctx.table(prefilled_catalog.factory.apply_prefix("simple_datatypes"))
    rb = pa.Table.from_batches(df.collect())
    assert rb.num_rows > 0

    # Get table by fully qualified name
    df = ctx.table(prefilled_catalog.factory.apply_prefix("datafusion.public.simple_datatypes"))
    rb = pa.Table.from_batches(df.collect())
    assert rb.num_rows > 0

    # Verify SQL parsing for catalog provider works as expected
    df = ctx.sql(f"SELECT * FROM {prefilled_catalog.factory.apply_prefix('simple_datatypes')}")
    rb = pa.Table.from_batches(df.collect())
    assert rb.num_rows > 0

    df = ctx.sql(f"SELECT * FROM {prefilled_catalog.factory.apply_prefix('datafusion.public.simple_datatypes')}")
    rb = pa.Table.from_batches(df.collect())
    assert rb.num_rows > 0
