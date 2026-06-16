from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest

if TYPE_CHECKING:
    from e2e_redap_tests.conftest import EntryFactory


def test_configure_blueprint_dataset(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """Test configuring a blueprint dataset."""
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"
    rbl_uri2 = resource_prefix + "blueprints/table_blueprint2.rbl"

    ds = entry_factory.create_dataset("my_new_dataset")
    ds.register_prefix(resource_prefix + "dataset").wait()

    bds = ds.blueprint_dataset()
    assert bds is not None

    ds.register_blueprint(rbl_uri)

    assert len(bds.segment_ids()) == 1
    first_blueprint_name = ds.default_blueprint()

    ds.register_blueprint(rbl_uri2, set_default=False)

    assert len(bds.segment_ids()) == 2
    assert first_blueprint_name == ds.default_blueprint()

    # Get the second blueprint name
    [second_blueprint_name] = list(set(bds.segment_ids()) - {first_blueprint_name})

    ds.set_default_blueprint(second_blueprint_name)
    assert second_blueprint_name == ds.default_blueprint()


def test_reregister_same_blueprint(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """Re-registering the same blueprint should succeed, not raise AlreadyExistsError (regression test for RR-3904)."""
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"

    ds = entry_factory.create_dataset("reregister_blueprint_dataset")
    ds.register_prefix(resource_prefix + "dataset").wait()

    ds.register_blueprint(rbl_uri)

    bds = ds.blueprint_dataset()
    assert bds is not None
    assert len(bds.segment_ids()) == 1

    # Re-register the exact same blueprint — this should not raise
    ds.register_blueprint(rbl_uri)


def test_configure_table_blueprint_dataset(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """Test configuring a table blueprint dataset."""
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"
    rbl_uri2 = resource_prefix + "blueprints/table_blueprint2.rbl"

    table = entry_factory.create_table("table_with_blueprints", pa.schema([pa.field("col", pa.int32())]))

    assert table.blueprint_dataset() is not None
    assert table.blueprints() == []
    assert table.default_blueprint() is None

    table.register_blueprint(rbl_uri)

    blueprint_dataset = table.blueprint_dataset()
    assert blueprint_dataset is not None
    assert len(blueprint_dataset.segment_ids()) == 1
    assert table.blueprints() == blueprint_dataset.segment_ids()

    first_blueprint_name = table.default_blueprint()
    assert first_blueprint_name is not None
    assert first_blueprint_name in table.blueprints()

    table.register_blueprint(rbl_uri2, set_default=False)

    assert len(table.blueprints()) == 2
    assert table.default_blueprint() == first_blueprint_name

    [second_blueprint_name] = list(set(table.blueprints()) - {first_blueprint_name})
    table.set_default_blueprint(second_blueprint_name)
    assert table.default_blueprint() == second_blueprint_name

    table.set_default_blueprint(None)
    assert table.default_blueprint() is None


def test_table_blueprint_set_default_false_creates_dataset_without_default(
    entry_factory: EntryFactory, resource_prefix: str
) -> None:
    """Registering the first table blueprint with set_default=False leaves default unset."""
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"

    table = entry_factory.create_table("table_blueprint_set_default_false", pa.schema([pa.field("col", pa.int32())]))

    assert table.blueprint_dataset() is not None
    assert table.default_blueprint() is None

    table.register_blueprint(rbl_uri, set_default=False)

    assert table.default_blueprint() is None
    blueprint_dataset = table.blueprint_dataset()
    assert blueprint_dataset is not None
    assert len(blueprint_dataset.segment_ids()) == 1
    assert table.blueprints() == blueprint_dataset.segment_ids()


def test_table_default_blueprint_uses_creation_blueprint_dataset(entry_factory: EntryFactory) -> None:
    """Setting a table default blueprint uses the dataset created with the table."""
    table = entry_factory.create_table("table_default_with_blueprint_dataset", pa.schema([pa.field("col", pa.int32())]))

    table.set_default_blueprint("missing_blueprint_segment")

    assert table.blueprint_dataset() is not None
    assert table.default_blueprint() == "missing_blueprint_segment"


def test_table_default_blueprint_rejects_deleted_blueprint_dataset(
    entry_factory: EntryFactory, resource_prefix: str
) -> None:
    """Setting a table default blueprint should fail if the referenced blueprint dataset is gone."""
    table = entry_factory.create_table("table_deleted_blueprint_dataset", pa.schema([pa.field("col", pa.int32())]))
    table.register_blueprint(resource_prefix + "blueprints/table_blueprint.rbl", set_default=False)

    blueprint_dataset = table.blueprint_dataset()
    assert blueprint_dataset is not None
    blueprint_dataset.delete()

    with pytest.raises(Exception, match="table blueprint dataset does not exist"):
        table.set_default_blueprint("missing_blueprint_segment")


def test_dataset_default_blueprint_rejects_deleted_blueprint_dataset(entry_factory: EntryFactory) -> None:
    """Setting a dataset default blueprint should fail if the referenced blueprint dataset is gone."""
    dataset = entry_factory.create_dataset("dataset_deleted_blueprint_dataset")
    blueprint_dataset = dataset.blueprint_dataset()
    assert blueprint_dataset is not None
    blueprint_dataset.delete()

    with pytest.raises(Exception, match="dataset blueprint dataset does not exist"):
        dataset.set_default_blueprint("missing_blueprint_segment")
