from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from syrupy import SnapshotAssertion

    from e2e_redap_tests.conftest import EntryFactory


def test_segment_ids(entry_factory: EntryFactory, resource_prefix: str, snapshot: SnapshotAssertion) -> None:
    """Test that we can successfully collect information about segments."""

    ds = entry_factory.create_dataset("test_dataset")
    handle = ds.register_prefix(resource_prefix + "dataset")
    handle.wait(timeout_secs=50)

    table = (
        ds.segment_table()
        .drop("rerun_storage_urls", "rerun_last_updated_at", "rerun_size_bytes")
        .sort("rerun_segment_id")
        .to_arrow_table()
    )

    # TODO(RR-3172): Whether we have version metadata or not is currently highly nondeterministic, so we need to filter
    # it out for the snapshot test. The existence of the `sort` call above changes whether the metadata shows up or not,
    # but not for oss server.
    no_metadata_schema = table.schema.with_metadata(None)
    table_without_schema = table.cast(no_metadata_schema)

    assert table_without_schema == snapshot
    assert sorted(ds.segment_ids()) == snapshot
