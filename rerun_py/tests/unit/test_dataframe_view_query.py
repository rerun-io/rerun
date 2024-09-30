from __future__ import annotations

import pytest
import rerun as rr
import rerun.blueprint.components as blueprint_components
from rerun import TimeInt
from rerun.blueprint.archetypes import DataframeQueryV2


def test_component_column_selector_explicit() -> None:
    selector = blueprint_components.ComponentColumnSelector(entity_path="entity/path", component="ComponentName")

    assert selector.entity_path == rr.datatypes.EntityPath("entity/path")
    assert selector.component == rr.datatypes.Utf8("ComponentName")


def test_component_column_selector_spec() -> None:
    selector = blueprint_components.ComponentColumnSelector("entity/path:ComponentName")

    assert selector.entity_path == rr.datatypes.EntityPath("entity/path")
    assert selector.component == rr.datatypes.Utf8("ComponentName")


def test_component_column_selector_fail() -> None:
    with pytest.raises(ValueError):
        blueprint_components.ComponentColumnSelector(entity_path="entity/path", component=None)

    with pytest.raises(ValueError):
        blueprint_components.ComponentColumnSelector(spec="entity/path:ComponentName", entity_path="entity/path")

    with pytest.raises(ValueError):
        blueprint_components.ComponentColumnSelector(spec="entity/path:ComponentName", component="ComponentName")

    with pytest.raises(ValueError):
        blueprint_components.ComponentColumnSelector()

    with pytest.raises(ValueError):
        blueprint_components.ComponentColumnSelector(spec="hello")

    with pytest.raises(ValueError):
        blueprint_components.ComponentColumnSelector(spec="hello:world:extra")


def test_component_column_selector_batch() -> None:
    a = blueprint_components.ComponentColumnSelectorBatch(["/entity/path:ComponentName"])
    b = blueprint_components.ComponentColumnSelectorBatch(
        blueprint_components.ComponentColumnSelector("/entity/path:ComponentName")
    )
    c = blueprint_components.ComponentColumnSelectorBatch([
        blueprint_components.ComponentColumnSelector("/entity/path:ComponentName")
    ])

    assert a == b
    assert b == c


def test_dataframe_query_property() -> None:
    query = DataframeQueryV2(
        timeline="frame",
        filter_by_range=(TimeInt(seq=1), TimeInt(seq=10)),
        filter_by_event="/entity/path:ComponentName",
        apply_latest_at=True,
    )

    # TODO: selected_columns

    assert query.timeline == blueprint_components.TimelineNameBatch("frame")
    assert query.range_filter == blueprint_components.RangeFilterBatch(
        blueprint_components.RangeFilter(rr.datatypes.TimeInt(seq=1), rr.datatypes.TimeInt(seq=10))
    )
    assert query.filter_by_event_active == blueprint_components.FilterByEventActiveBatch(
        blueprint_components.FilterByEventActive(True)
    )
    assert query.filter_by_event_column == blueprint_components.ComponentColumnSelectorBatch(
        blueprint_components.ComponentColumnSelector(entity_path="/entity/path", component="ComponentName")
    )
    assert query.apply_latest_at == blueprint_components.ApplyLatestAtBatch(blueprint_components.ApplyLatestAt(True))


def test_dataframe_query_property_explicit() -> None:
    query = DataframeQueryV2(
        timeline=blueprint_components.TimelineName("frame"),
        filter_by_range=blueprint_components.RangeFilter(start=TimeInt(seq=1), end=TimeInt(seq=10)),
        filter_by_event=blueprint_components.ComponentColumnSelector(
            entity_path="/entity/path", component="ComponentName"
        ),
    )

    # TODO: selected_columns

    assert query.timeline == blueprint_components.TimelineNameBatch("frame")
    assert query.range_filter == blueprint_components.RangeFilterBatch(
        blueprint_components.RangeFilter(rr.datatypes.TimeInt(seq=1), rr.datatypes.TimeInt(seq=10))
    )
    assert query.filter_by_event_active == blueprint_components.FilterByEventActiveBatch(
        blueprint_components.FilterByEventActive(True)
    )
    assert query.filter_by_event_column == blueprint_components.ComponentColumnSelectorBatch(
        blueprint_components.ComponentColumnSelector(entity_path="/entity/path", component="ComponentName")
    )
