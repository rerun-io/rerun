from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest

if TYPE_CHECKING:
    import pathlib

    from e2e_redap_tests.conftest import EntryFactory, PrefilledCatalog


@pytest.mark.creates_table
def test_create_table(entry_factory: EntryFactory, tmp_path: pathlib.Path) -> None:
    table_name = "created_table"

    original_schema = pa.schema([("int64", pa.int64()), ("float32", pa.float32()), ("utf8", pa.utf8())])

    table_entry = entry_factory.create_table_entry(table_name, original_schema, tmp_path.absolute().as_uri())
    df = table_entry.df()

    returned_schema = df.schema().remove_metadata()
    assert returned_schema == original_schema


@pytest.mark.creates_table
def test_create_table_from_dataset(prefilled_catalog: PrefilledCatalog, tmp_path: pathlib.Path) -> None:
    table_name = "dataset_to_table"

    df = prefilled_catalog.dataset.dataframe_query_view(index="time_1", contents="/**").df()
    original_schema = df.schema()

    table_entry = prefilled_catalog.factory.create_table_entry(
        table_name, original_schema, tmp_path.absolute().as_uri()
    )
    df = table_entry.df()

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

@pytest.mark.aws_ci_credentials
def test_create_table_on_s3(entry_factory: EntryFactory, resource_prefix: str) -> None:
    table_name = "created_table"
    table_location = f"{resource_prefix}{table_name}"

    original_schema = pa.schema([("int64", pa.int64()), ("float32", pa.float32()), ("utf8", pa.utf8())])
    table_entry = entry_factory.create_table_entry(table_name, original_schema, table_location)
    df = table_entry.df()

    returned_schema = df.schema().remove_metadata()
    assert returned_schema == original_schema
