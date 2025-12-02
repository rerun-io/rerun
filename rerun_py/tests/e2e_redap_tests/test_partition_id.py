from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from syrupy import SnapshotAssertion

    from e2e_redap_tests.conftest import EntryFactory


def test_partition_ids(entry_factory: EntryFactory, resource_prefix: str, snapshot: SnapshotAssertion) -> None:
    """Test that we can successfully collect information about segments."""

    ds = entry_factory.create_dataset("test_dataset")
    tasks = ds.register_prefix(resource_prefix + "dataset")
    tasks.wait(timeout_secs=50)

    assert (
        ds.segment_table()
        .df()
        .drop("rerun_storage_urls", "rerun_last_updated_at", "rerun_size_bytes")
        .sort("rerun_segment_id")
        == snapshot
    )
    assert sorted(ds.segment_ids()) == snapshot
