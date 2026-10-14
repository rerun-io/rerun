from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from rerun.catalog import NotFoundError, OnDuplicateSegmentLayer, Segment, UnregistrationHandle

if TYPE_CHECKING:
    from syrupy import SnapshotAssertion

    from e2e_redap_tests.conftest import EntryFactory


def test_segment_ids(entry_factory: EntryFactory, resource_prefix: str, snapshot: SnapshotAssertion) -> None:
    """Test that we can successfully collect information about segments."""

    ds = entry_factory.create_dataset("test_dataset")
    handle = ds.register_prefix(resource_prefix + "dataset")
    handle.wait(timeout_secs=50)

    table = (
        ds
        .segment_table()
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


def test_unknown_segment(entry_factory: EntryFactory) -> None:
    ds = entry_factory.create_dataset("unknown_segment")
    with pytest.raises(NotFoundError, match="not-registered"):
        ds.segment("not-registered")
    assert ds.segment_ids() == []


def test_segment_layers(entry_factory: EntryFactory, resource_prefix: str) -> None:
    ds = entry_factory.create_dataset("segment_layers")
    segment_ids = sorted(ds.register_prefix(resource_prefix + "dataset").wait().segment_ids)
    assert len(segment_ids) > 1

    segment = ds.segment(segment_ids[-1])
    assert isinstance(segment, Segment)
    assert segment.id == segment_ids[-1]
    assert str(segment) == segment.id
    assert segment.dataset.id == ds.id
    assert segment.dataset._internal is ds._internal
    ds.set_name(entry_factory.apply_prefix("renamed_segment_layers"))
    assert segment.dataset.name == ds.name
    assert segment.url == ds.segment_url(segment.id)
    assert set(segment.layers()) == {"base"}
    ds.register_prefix(resource_prefix + "dataset", layer_name="predictions").wait()
    assert set(segment.layers()) == {"base", "predictions"}

    with pytest.raises(ValueError, match="must not be empty"):
        segment.delete_layer("")

    deletion = segment.delete_layer("predictions")
    assert isinstance(deletion, UnregistrationHandle)
    deletion.wait()
    assert set(segment.layers()) == {"base"}
    for segment_id in segment_ids[:-1]:
        assert set(ds.segment(segment_id).layers()) == {"base", "predictions"}

    segment.delete_layer("predictions").wait()
    segment.delete_layer("base").wait()
    with pytest.raises(NotFoundError):
        segment.layers()
    with pytest.raises(NotFoundError):
        ds.segment(segment.id)
    segment.delete_layer("base").wait()

    ds.register_prefix(resource_prefix + "dataset", on_duplicate=OnDuplicateSegmentLayer.SKIP).wait()
    assert set(segment.layers()) == {"base"}
