from __future__ import annotations

import dataclasses
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry, DatasetInfo
    from syrupy import SnapshotAssertion

    from e2e_redap_tests.conftest import EntryFactory


def _stable(info: DatasetInfo) -> DatasetInfo:
    """Clear the fields that vary with the test session or the server."""
    return dataclasses.replace(info, dataset_name="<dataset>", size_bytes=0)


def test_dataset_info(readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion) -> None:
    info = readonly_test_dataset.info()

    assert info.size_bytes > 0
    assert repr(_stable(info)) == snapshot


def test_dataset_info_layers_and_properties(entry_factory: EntryFactory, resource_prefix: str) -> None:
    dataset = entry_factory.create_dataset("dataset_info_layers")
    dataset.register([
        resource_prefix + f"segment_schema/{segment_id}.rrd" for segment_id in ("first", "second")
    ]).wait()
    dataset.register([resource_prefix + "segment_schema/predictions.rrd"], layer_name="predictions").wait()

    info = dataset.info()

    assert info.num_segments == 2
    assert {layer.name: layer.num_segments for layer in info.layers} == {"base": 2, "predictions": 1}
    assert info.property_columns == frozenset({"property:RecordingInfo:start_time"})


def test_dataset_info_empty(entry_factory: EntryFactory, snapshot: SnapshotAssertion) -> None:
    info = entry_factory.create_dataset("empty_dataset").info()

    assert repr(_stable(info)) == snapshot
