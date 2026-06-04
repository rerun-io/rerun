from __future__ import annotations

import uuid
from typing import TYPE_CHECKING

import pytest
from rerun.catalog import NotFoundError

if TYPE_CHECKING:
    from rerun.catalog import CatalogClient


def test_delete_dataset_removes_catalog_entry(catalog_client: CatalogClient, resource_prefix: str) -> None:
    """Deleting a dataset removes it from catalog lookup and listing."""
    dataset_name = f"test_delete_dataset_{uuid.uuid4().hex}"
    dataset = catalog_client.create_dataset(dataset_name)
    deleted = False

    try:
        handle = dataset.register_prefix(resource_prefix + "dataset")
        handle.wait(timeout_secs=50)
        assert dataset.segment_ids()

        dataset_id = dataset.id
        blueprint_dataset = dataset.blueprint_dataset()
        assert blueprint_dataset is not None
        blueprint_dataset_id = blueprint_dataset.id

        dataset.delete()
        deleted = True

        with pytest.raises(LookupError):
            catalog_client.get_dataset(dataset_name)
        with pytest.raises(NotFoundError):
            catalog_client.get_dataset(id=dataset_id)
        with pytest.raises(NotFoundError):
            catalog_client.get_dataset(id=blueprint_dataset_id)

        assert dataset_name not in catalog_client.dataset_names()
        assert dataset_name not in catalog_client.entry_names()
        assert all(entry.id != dataset_id for entry in catalog_client.entries(include_hidden=True))
        assert all(entry.id != blueprint_dataset_id for entry in catalog_client.entries(include_hidden=True))
    finally:
        if not deleted:
            dataset.delete()
