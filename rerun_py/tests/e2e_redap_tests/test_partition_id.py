from __future__ import annotations

from typing import TYPE_CHECKING

from .conftest import DATASET_FILEPATH

if TYPE_CHECKING:
    from rerun.catalog import CatalogClient
    from syrupy import SnapshotAssertion


def test_partition_ids(catalog_client: CatalogClient, snapshot: SnapshotAssertion) -> None:
    """Test that we can successfully collect information about partitions."""

    ds = catalog_client.create_dataset("test_dataset")
    tasks = ds.register_prefix(DATASET_FILEPATH.absolute().as_uri())
    tasks.wait(timeout_secs=50)

    assert (
        ds.partition_table().df().drop("rerun_storage_urls", "rerun_last_updated_at").sort("rerun_partition_id")
        == snapshot
    )
    assert sorted(ds.partition_ids()) == snapshot
