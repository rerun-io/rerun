from __future__ import annotations

from typing import TYPE_CHECKING

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
