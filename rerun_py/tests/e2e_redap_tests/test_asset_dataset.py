from __future__ import annotations

import uuid
from typing import TYPE_CHECKING

import pytest
from rerun.catalog import EntryKind, NotFoundError

if TYPE_CHECKING:
    from collections.abc import Callable, Sequence

    from rerun.catalog import CatalogClient

    from e2e_redap_tests.conftest import EntryFactory


@pytest.mark.local_only
def test_register_asset_appears_in_asset_dataset(
    entry_factory: EntryFactory,
    static_recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Registering an asset puts it in the dataset's asset dataset and returns its segment id."""
    recording_id = "registered_asset"
    [uri] = static_recording_factory([recording_id])

    ds = entry_factory.create_dataset("dataset_with_asset")

    asset_dataset = ds.asset_dataset()
    assert asset_dataset is not None
    assert asset_dataset.segment_ids() == []

    segment_id = ds.register_asset(uri)

    assert segment_id == recording_id
    assert asset_dataset.segment_ids() == [recording_id]


@pytest.mark.local_only
def test_register_asset_replaces_duplicate(
    entry_factory: EntryFactory,
    static_recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Registering the same asset twice replaces it instead of raising."""
    recording_id = "duplicate_asset"
    uris = static_recording_factory([recording_id, recording_id])

    ds = entry_factory.create_dataset("dataset_with_replaced_asset")

    assert ds.register_asset(uris[0]) == recording_id
    assert ds.register_asset(uris[1]) == recording_id

    asset_dataset = ds.asset_dataset()
    assert asset_dataset is not None
    assert asset_dataset.segment_ids() == [recording_id]


@pytest.mark.local_only
def test_unregister_asset_removes_it_from_asset_dataset(
    entry_factory: EntryFactory,
    static_recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Unregistering an asset removes it from the dataset's asset dataset."""
    recording_id = "asset_to_unregister"
    [uri] = static_recording_factory([recording_id])

    ds = entry_factory.create_dataset("dataset_with_unregistered_asset")

    segment_id = ds.register_asset(uri)
    asset_dataset = ds.asset_dataset()
    assert asset_dataset is not None
    assert asset_dataset.segment_ids() == [recording_id]

    ds.unregister_asset(segment_id)

    assert asset_dataset.segment_ids() == []


@pytest.mark.local_only
def test_unregister_unknown_asset_is_noop(entry_factory: EntryFactory) -> None:
    """Unregistering an asset that was never registered does nothing."""
    ds = entry_factory.create_dataset("dataset_with_noop_unregister")

    ds.unregister_asset("never-registered-segment")

    asset_dataset = ds.asset_dataset()
    assert asset_dataset is not None
    assert asset_dataset.segment_ids() == []


def test_deleting_dataset_deletes_asset_dataset(catalog_client: CatalogClient) -> None:
    """Creating a dataset creates an asset dataset of the right kind, and deleting the dataset deletes it too."""
    dataset_name = f"dataset_with_asset_{uuid.uuid4().hex}"
    dataset = catalog_client.create_dataset(dataset_name)
    deleted = False

    try:
        asset_dataset = dataset.asset_dataset()
        assert asset_dataset is not None
        assert asset_dataset.kind == EntryKind.ASSET_DATASET
        asset_dataset_id = asset_dataset.id

        dataset.delete()
        deleted = True

        with pytest.raises(NotFoundError):
            catalog_client.get_dataset(id=asset_dataset_id)
        assert all(entry.id != asset_dataset_id for entry in catalog_client.entries(include_hidden=True))
    finally:
        if not deleted:
            dataset.delete()
