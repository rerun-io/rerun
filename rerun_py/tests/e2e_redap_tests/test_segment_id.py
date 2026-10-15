from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
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
    schema = segment.schema()
    assert schema.to_arrow().equals(pa.schema(schema), check_metadata=True)
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
        segment.schema()
    with pytest.raises(NotFoundError):
        ds.segment(segment.id)
    segment.delete_layer("base").wait()

    ds.register_prefix(resource_prefix + "dataset", on_duplicate=OnDuplicateSegmentLayer.SKIP).wait()
    assert set(segment.layers()) == {"base"}
    assert segment.schema() == schema


def test_segment_schema(entry_factory: EntryFactory, resource_prefix: str) -> None:
    ds = entry_factory.create_dataset("segment_schema")
    for segment_id in ("first", "second"):
        ds.register([resource_prefix + f"segment_schema/{segment_id}.rrd"]).wait()

    segment = ds.segment("first")
    schema = segment.schema()
    assert schema.entity_paths() == ["/first", "/static"]
    assert "first" in {column.name for column in schema.index_columns()}
    assert "second" not in {column.name for column in schema.index_columns()}
    assert any(column.is_static for column in schema.component_columns())
    assert any(not column.is_static for column in schema.component_columns())
    arrow_schema = schema.to_arrow()
    assert arrow_schema.equals(pa.schema(schema), check_metadata=True)
    assert all(arrow_schema.field(column.name).metadata for column in schema)

    dataset_schema = ds.schema()
    assert dataset_schema.entity_paths() == ["/first", "/second", "/static"]
    assert dataset_schema.to_arrow().equals(pa.schema(dataset_schema), check_metadata=True)
    view_schema = ds.filter_contents(["/first"]).schema()
    assert view_schema.entity_paths() == ["/first"]
    assert view_schema.to_arrow().equals(pa.schema(view_schema), check_metadata=True)

    ds.register([resource_prefix + "segment_schema/predictions.rrd"], layer_name="predictions").wait()
    assert segment.schema().entity_paths() == ["/first", "/predictions", "/static"]
    assert schema.entity_paths() == ["/first", "/static"]

    segment.delete_layer("predictions").wait()
    assert segment.schema() == schema
    ds.delete()
    with pytest.raises(NotFoundError):
        segment.schema()
    assert schema.to_arrow().equals(arrow_schema, check_metadata=True)
    assert dataset_schema.to_arrow().equals(pa.schema(dataset_schema), check_metadata=True)
    assert view_schema.to_arrow().equals(pa.schema(view_schema), check_metadata=True)
