from __future__ import annotations

import pytest
import rerun as rr
import rerun.blueprint.components as blueprint_components
from rerun import TimeInt, datatypes
from rerun.blueprint.archetypes import DataframeQuery


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
        blueprint_components.ComponentColumnSelector("/entity/path:ComponentName"),
    )
    c = blueprint_components.ComponentColumnSelectorBatch([
        blueprint_components.ComponentColumnSelector("/entity/path:ComponentName"),
    ])

    assert a == b
    assert b == c


def test_selected_columns() -> None:
    columns = blueprint_components.SelectedColumns([
        "t",
        "/entity/path:ComponentName",
        datatypes.Utf8("frame"),
        blueprint_components.ComponentColumnSelector("/world/robot:Position3D"),
    ])

    assert columns.time_columns == [
        datatypes.Utf8("t"),
        datatypes.Utf8("frame"),
    ]
    assert columns.component_columns == [
        blueprint_components.ComponentColumnSelector("/entity/path:ComponentName"),
        blueprint_components.ComponentColumnSelector("/world/robot:Position3D"),
    ]


def test_selected_columns_batch() -> None:
    a = blueprint_components.SelectedColumnsBatch([
        [
            "t",
            "/entity/path:ComponentName",
            datatypes.Utf8("frame"),
            blueprint_components.ComponentColumnSelector("/world/robot:Position3D"),
        ],
    ])
    b = blueprint_components.SelectedColumnsBatch(
        blueprint_components.SelectedColumns([
            "t",
            "/entity/path:ComponentName",
            datatypes.Utf8("frame"),
            blueprint_components.ComponentColumnSelector("/world/robot:Position3D"),
        ]),
    )

    assert a == b


def test_selected_columns_batch_multiple() -> None:
    a = blueprint_components.SelectedColumnsBatch([
        [
            "t",
        ],
        [
            "/entity/path:ComponentName",
        ],
        [
            "frame",
            "/world/robot:Position3D",
        ],
    ])
    b = blueprint_components.SelectedColumnsBatch([
        blueprint_components.SelectedColumns([
            "t",
        ]),
        blueprint_components.SelectedColumns([
            "/entity/path:ComponentName",
        ]),
        blueprint_components.SelectedColumns([
            datatypes.Utf8("frame"),
            blueprint_components.ComponentColumnSelector("/world/robot:Position3D"),
        ]),
    ])

    assert a == b


def test_dataframe_query_property() -> None:
    query = DataframeQuery(
        timeline="frame",
        filter_by_range=(TimeInt(seq=1), TimeInt(seq=10)),
        filter_is_not_null="/entity/path:ComponentName",
        apply_latest_at=True,
        select=[
            "t",
            "/entity/path:ComponentName",
        ],
    )

    assert query.timeline == blueprint_components.TimelineNameBatch("frame")
    assert query.filter_by_range == blueprint_components.FilterByRangeBatch(
        blueprint_components.FilterByRange(rr.datatypes.TimeInt(seq=1), rr.datatypes.TimeInt(seq=10)),
    )
    assert query.filter_is_not_null == blueprint_components.FilterIsNotNullBatch(
        blueprint_components.FilterIsNotNull(
            active=True,
            column=blueprint_components.ComponentColumnSelector(entity_path="/entity/path", component="ComponentName"),
        ),
    )

    assert query.apply_latest_at == blueprint_components.ApplyLatestAtBatch(blueprint_components.ApplyLatestAt(True))

    assert query.select == blueprint_components.SelectedColumnsBatch(
        blueprint_components.SelectedColumns([
            datatypes.Utf8("t"),
            blueprint_components.ComponentColumnSelector(entity_path="/entity/path", component="ComponentName"),
        ]),
    )


def test_dataframe_query_property_explicit() -> None:
    query = DataframeQuery(
        timeline=blueprint_components.TimelineName("frame"),
        filter_by_range=blueprint_components.FilterByRange(start=TimeInt(seq=1), end=TimeInt(seq=10)),
        filter_is_not_null=blueprint_components.ComponentColumnSelector(
            entity_path="/entity/path",
            component="ComponentName",
        ),
        select=[
            datatypes.Utf8("frame"),
            blueprint_components.ComponentColumnSelector("/world/robot:Position3D"),
        ],
    )

    assert query.timeline == blueprint_components.TimelineNameBatch("frame")
    assert query.filter_by_range == blueprint_components.FilterByRangeBatch(
        blueprint_components.FilterByRange(rr.datatypes.TimeInt(seq=1), rr.datatypes.TimeInt(seq=10)),
    )
    assert query.filter_is_not_null == blueprint_components.FilterIsNotNullBatch(
        blueprint_components.FilterIsNotNull(
            active=True,
            column=blueprint_components.ComponentColumnSelector(entity_path="/entity/path", component="ComponentName"),
        ),
    )

    assert query.select == blueprint_components.SelectedColumnsBatch(
        blueprint_components.SelectedColumns([
            datatypes.Utf8("frame"),
            blueprint_components.ComponentColumnSelector(entity_path="/world/robot", component="Position3D"),
        ]),
    )
