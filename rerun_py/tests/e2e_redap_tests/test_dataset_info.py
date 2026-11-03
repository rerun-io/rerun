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


def test_dataset_selection_info(entry_factory: EntryFactory, resource_prefix: str) -> None:
    dataset = entry_factory.create_dataset("dataset_selection_info")
    dataset.register([
        resource_prefix + f"segment_schema/{segment_id}.rrd" for segment_id in ("first", "second")
    ]).wait()
    dataset.register([resource_prefix + "segment_schema/predictions.rrd"], layer_name="predictions").wait()
    full = dataset.info()

    assert dataset.filter_contents("/**").info() == full

    first = dataset.filter_segments("first").info()
    assert first.num_segments == 1
    assert 0 < first.size_bytes < full.size_bytes
    assert {layer.name: layer.num_segments for layer in first.layers} == {"base": 1, "predictions": 1}
    timelines = {timeline.name: timeline.num_segments for timeline in first.timelines}
    assert timelines["first"] == 1
    assert "second" not in timelines
    assert first.num_entities == 3  # /first, /predictions and /static
    assert first.property_columns == full.property_columns

    # A content filter changes only the entity and component column counts.
    second = dataset.filter_segments("second")
    second_static = second.filter_contents("/static").info()
    assert second_static.num_entities == 1
    assert dataclasses.replace(second_static, num_entities=0, num_component_columns=0) == dataclasses.replace(
        second.info(), num_entities=0, num_component_columns=0
    )

    for nothing in (dataset.filter_segments([]).info(), dataset.filter_segments("unknown").info()):
        assert nothing.num_segments == 0
        assert nothing.size_bytes == 0
        assert nothing.layers == frozenset()
        assert nothing.timelines == frozenset()
        assert nothing.num_entities == 0
        assert nothing.property_columns == full.property_columns
