from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from rerun.catalog import Layer, NotFoundError

if TYPE_CHECKING:
    from e2e_redap_tests.conftest import EntryFactory


def test_unknown_layer(entry_factory: EntryFactory) -> None:
    ds = entry_factory.create_dataset("unknown_layer")
    with pytest.raises(ValueError, match="must not be empty"):
        ds.layer("")
    with pytest.raises(ValueError, match=r"Available layers: \[\]"):
        ds.layer("unknown")
    ds.delete()
    with pytest.raises(NotFoundError):
        ds.layer("unknown")


def test_layer_handle(entry_factory: EntryFactory, resource_prefix: str) -> None:
    ds = entry_factory.create_dataset("layer_handle")
    ids = sorted(ds.register_prefix(resource_prefix + "dataset").wait().segment_ids)
    assert len(ids) > 1
    base = ds.layer("base")
    assert isinstance(base, Layer)
    assert base.name == "base"
    assert str(base) == "base"
    assert "base" in repr(base)
    assert base.dataset.id == ds.id
    assert base.dataset._internal is ds._internal
    assert sorted(base.segment_ids()) == ids
    with pytest.raises(ValueError, match=r'Available layers: \["base"\]'):
        ds.layer("predictions")

    ds.register_prefix(resource_prefix + "dataset", layer_name="predictions").wait()
    predictions = ds.layer("predictions")
    assert sorted(predictions.segment_ids()) == ids
    ds.set_name(entry_factory.apply_prefix("renamed_layer_handle"))
    assert predictions.dataset.name == ds.name

    ds.segment(ids[0]).delete_layer("predictions").wait()
    assert sorted(predictions.segment_ids()) == ids[1:]
    assert sorted(base.segment_ids()) == ids
    for segment_id in ids[1:]:
        ds.segment(segment_id).delete_layer("predictions").wait()
    assert predictions.segment_ids() == []
    with pytest.raises(ValueError, match=r'Available layers: \["base"\]'):
        ds.layer("predictions")

    ds.register_prefix(resource_prefix + "dataset", layer_name="predictions").wait()
    del ds
    assert sorted(predictions.segment_ids()) == ids
    predictions.dataset.delete()
    with pytest.raises(NotFoundError):
        predictions.segment_ids()


def test_layer_schema(entry_factory: EntryFactory, resource_prefix: str) -> None:
    ds = entry_factory.create_dataset("layer_schema")
    for segment_id in ("first", "second"):
        ds.register([resource_prefix + f"segment_schema/{segment_id}.rrd"]).wait()
    ds.register([resource_prefix + "segment_schema/predictions.rrd"], layer_name="predictions").wait()

    base = ds.layer("base")
    base_schema = base.schema()
    assert base_schema.entity_paths() == ["/first", "/second", "/static"]
    assert {column.name for column in base_schema.index_columns()} >= {"first", "second"}
    assert base_schema.to_arrow().equals(pa.schema(base_schema), check_metadata=True)

    predictions = ds.layer("predictions")
    assert predictions.schema().entity_paths() == ["/predictions"]
    assert ds.schema().entity_paths() == ["/first", "/predictions", "/second", "/static"]

    ds.segment("first").delete_layer("predictions").wait()
    with pytest.raises(NotFoundError):
        predictions.schema()
    assert base.schema() == base_schema

    ds.delete()
    with pytest.raises(NotFoundError):
        base.schema()
