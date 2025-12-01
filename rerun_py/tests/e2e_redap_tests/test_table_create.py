from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest

if TYPE_CHECKING:
    import pathlib

    from rerun.catalog import CatalogClient

    from e2e_redap_tests.conftest import EntryFactory, PrefilledCatalog


@pytest.mark.creates_table
def test_create_table(entry_factory: EntryFactory, tmp_path: pathlib.Path) -> None:
    table_name = "created_table"

    original_schema = pa.schema([("int64", pa.int64()), ("float32", pa.float32()), ("utf8", pa.utf8())])

    table_entry = entry_factory.create_table(table_name, original_schema, tmp_path.absolute().as_uri())
    df = table_entry.reader()

    returned_schema = df.schema().remove_metadata()
    assert returned_schema == original_schema


@pytest.mark.creates_table
def test_create_table_from_dataset(prefilled_catalog: PrefilledCatalog, tmp_path: pathlib.Path) -> None:
    table_name = "dataset_to_table"

    df = prefilled_catalog.prefilled_dataset.reader(index="time_1")
    original_schema = df.schema()

    table_entry = prefilled_catalog.factory.create_table(table_name, original_schema, tmp_path.absolute().as_uri())
    df = table_entry.reader()

    # Due to https://github.com/lance-format/lance/issues/2304 we cannot
    # directly compare the returned schema. Verify we at least
    # get back the same columns and metadata

    returned_schema = df.schema()
    for field in returned_schema:
        assert original_schema.field(field.name) is not None
    for field in original_schema:
        assert returned_schema.field(field.name) is not None

    for returned_field in returned_schema:
        original_field = original_schema.field(returned_field.name)
        assert returned_field.metadata == original_field.metadata


def test_create_table_in_custom_schema(catalog_client: CatalogClient, tmp_path: pathlib.Path) -> None:
    table_name = "my_catalog.my_schema.created_table"

    original_schema = pa.schema([("int64", pa.int64()), ("float32", pa.float32()), ("utf8", pa.utf8())])

    table_entry = catalog_client.create_table(table_name, original_schema, tmp_path.absolute().as_uri())

    try:
        df = catalog_client.ctx.catalog("my_catalog").schema("my_schema").table("created_table")

        returned_schema = df.schema.remove_metadata()
        assert returned_schema == original_schema
    finally:
        table_entry.delete()


@pytest.mark.creates_table
def test_create_table_invalid_name(entry_factory: EntryFactory, tmp_path: pathlib.Path) -> None:
    table_name = "created-table"

    schema = pa.schema([("int64", pa.int64()), ("float32", pa.float32()), ("utf8", pa.utf8())])
    with pytest.raises(
        ValueError,
        match="sql parser error: Unexpected token in identifier: -",
    ):
        _ = entry_factory.create_table(table_name, schema, tmp_path.absolute().as_uri())

def test_create_existing_table_fails(
    prefilled_catalog: PrefilledCatalog, entry_factory: EntryFactory, tmp_path: pathlib.Path
) -> None:
    from .conftest import TABLE_FILEPATH

    existing_table_name = "simple_datatypes"

    _existing_table = prefilled_catalog.client.ctx.table(entry_factory.apply_prefix(existing_table_name))

    schema = pa.schema([("int64", pa.int64()), ("float32", pa.float32()), ("utf8", pa.utf8())])

    with pytest.raises(
        Exception,
        match="failed to create table",
    ):
        _table_entry = entry_factory.create_table_entry(existing_table_name, schema, tmp_path.absolute().as_uri())

    existing_table_location = f"file://{TABLE_FILEPATH}"

    with pytest.raises(
        Exception,
        match="failed to create table",
    ):
        _table_entry = entry_factory.create_table_entry("new_table_name", schema, existing_table_location)
