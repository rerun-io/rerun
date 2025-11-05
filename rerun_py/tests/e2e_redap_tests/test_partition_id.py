from __future__ import annotations

from typing import TYPE_CHECKING

from .conftest import DATASET_FILEPATH, ServerInstance

if TYPE_CHECKING:
    from syrupy import SnapshotAssertion


def test_partition_ids(server_instance: ServerInstance, snapshot: SnapshotAssertion) -> None:
    """Test that we can successfully collect information about partitions."""
    client = server_instance.client

    ds = client.create_dataset("test_dataset")
    tasks = ds.register_prefix(f"file://{DATASET_FILEPATH.absolute()}")
    tasks.wait(timeout_secs=50)

    assert (
        ds.partition_table().df().drop("rerun_storage_urls", "rerun_last_updated_at").sort("rerun_partition_id")
        == snapshot
    )
    assert sorted(ds.partition_ids()) == snapshot
