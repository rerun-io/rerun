"""Tests for rerun.recording module (non-deprecated Recording functionality)."""

from __future__ import annotations

import pathlib
import uuid
from typing import TYPE_CHECKING

import rerun as rr

if TYPE_CHECKING:
    import syrupy

APP_ID = "rerun_example_test_recording"


def test_recording_info(tmp_path: pathlib.Path) -> None:
    """Test Recording.application_id() and Recording.recording_id()."""

    rrd = tmp_path / "tmp.rrd"

    expected_recording_id = uuid.uuid4()
    with rr.RecordingStream(APP_ID, recording_id=expected_recording_id) as rec:
        rec.save(rrd)
        rec.set_time("my_index", sequence=1)
        rec.log("points", rr.Points3D([[1, 2, 3]]))

    recording = rr.recording.load_recording(rrd)

    assert recording.application_id() == APP_ID
    assert recording.recording_id() == str(expected_recording_id)


def test_schema_recording(tmp_path: pathlib.Path, snapshot: syrupy.SnapshotAssertion) -> None:
    """Test Recording.schema() returns correct index and component columns."""

    rrd = tmp_path / "tmp.rrd"

    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(rrd)
        rec.set_time("my_index", sequence=1)
        rec.log("points", rr.Points3D([[1, 2, 3], [4, 5, 6], [7, 8, 9]], radii=[]))
        rec.set_time("my_index", sequence=7)
        rec.log("points", rr.Points3D([[10, 11, 12]], colors=[[255, 0, 0]]))
        rec.log("static_text", rr.TextLog("Hello"), static=True)

    recording = rr.recording.load_recording(rrd)
    schema = recording.schema()

    # log_tick, log_time, my_index
    assert len(schema.index_columns()) == 3
    # Timestamp, Color, Position3D, Radius, Text
    assert len(schema.component_columns()) == 5

    # Index columns
    assert schema.index_columns()[0].name == "log_tick"
    assert schema.index_columns()[1].name == "log_time"
    assert schema.index_columns()[2].name == "my_index"

    assert str(schema) == snapshot()

    col = 0

    # Content columns
    assert schema.component_columns()[col].entity_path == "/points"
    assert schema.component_columns()[col].archetype == "rerun.archetypes.Points3D"
    assert schema.component_columns()[col].component == "Points3D:colors"
    assert schema.component_columns()[col].component_type == "rerun.components.Color"
    assert schema.component_columns()[col].is_static is False
    col += 1

    assert schema.component_columns()[col].entity_path == "/points"
    assert schema.component_columns()[col].archetype == "rerun.archetypes.Points3D"
    assert schema.component_columns()[col].component == "Points3D:positions"
    assert schema.component_columns()[col].component_type == "rerun.components.Position3D"
    assert schema.component_columns()[col].is_static is False
    col += 1

    assert schema.component_columns()[col].entity_path == "/points"
    assert schema.component_columns()[col].archetype == "rerun.archetypes.Points3D"
    assert schema.component_columns()[col].component == "Points3D:radii"
    assert schema.component_columns()[col].component_type == "rerun.components.Radius"
    assert schema.component_columns()[col].is_static is False
    col += 1

    assert schema.component_columns()[col].entity_path == "/static_text"
    assert schema.component_columns()[col].archetype == "rerun.archetypes.TextLog"
    assert schema.component_columns()[col].component == "TextLog:text"
    assert schema.component_columns()[col].component_type == "rerun.components.Text"
    assert schema.component_columns()[col].is_static is True
    col += 1

    assert schema.component_columns()[col].entity_path == "/__properties"
    assert schema.component_columns()[col].archetype == "rerun.archetypes.RecordingInfo"
    assert schema.component_columns()[col].component == "RecordingInfo:start_time"
    assert schema.component_columns()[col].component_type == "rerun.components.Timestamp"
    assert schema.component_columns()[col].is_static is True


def test_schema_entity_paths(tmp_path: pathlib.Path) -> None:
    """Test Schema.entity_paths() returns a sorted list of unique entity paths."""

    rrd = tmp_path / "tmp.rrd"

    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(rrd)
        rec.set_time("my_index", sequence=1)
        rec.log("points", rr.Points3D([[1, 2, 3]]))
        rec.log("static_text", rr.TextLog("Hello"), static=True)
        rec.send_property("my_prop", rr.AnyValues(prop=123))

    recording = rr.recording.load_recording(rrd)
    schema = recording.schema()

    assert schema.entity_paths() == ["/points", "/static_text"]
    assert schema.entity_paths(include_properties=True) == [
        "/__properties",
        "/__properties/my_prop",
        "/points",
        "/static_text",
    ]


def test_schema_archetypes(tmp_path: pathlib.Path) -> None:
    """Test Schema.archetypes() returns a sorted list of unique archetype names."""

    rrd = tmp_path / "tmp.rrd"

    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(rrd)
        rec.set_time("my_index", sequence=1)
        rec.log("points", rr.Points3D([[1, 2, 3]]))
        rec.log("static_text", rr.TextLog("Hello"), static=True)
        rec.send_property("my_prop", rr.Points2D([[0, 2]]))

    recording = rr.recording.load_recording(rrd)
    schema = recording.schema()

    assert schema.archetypes() == ["rerun.archetypes.Points3D", "rerun.archetypes.TextLog"]
    assert schema.archetypes(include_properties=True) == [
        "rerun.archetypes.Points2D",
        "rerun.archetypes.Points3D",
        "rerun.archetypes.RecordingInfo",
        "rerun.archetypes.TextLog",
    ]


def test_schema_component_types(tmp_path: pathlib.Path) -> None:
    """Test Schema.component_types() returns a sorted list of unique component types."""

    rrd = tmp_path / "tmp.rrd"

    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(rrd)
        rec.set_time("my_index", sequence=1)
        rec.log("points", rr.Points3D([[1, 2, 3]]))
        rec.log("static_text", rr.TextLog("Hello"), static=True)
        rec.send_property("my_prop", rr.Points2D([[0, 2]]))

    recording = rr.recording.load_recording(rrd)
    schema = recording.schema()

    assert schema.component_types() == [
        "rerun.components.Position3D",
        "rerun.components.Text",
    ]
    assert schema.component_types(include_properties=True) == [
        "rerun.components.Position2D",
        "rerun.components.Position3D",
        "rerun.components.Text",
        "rerun.components.Timestamp",
    ]


def test_schema_columns_for(tmp_path: pathlib.Path) -> None:
    """Test Schema.columns_for() filters component columns by entity_path, archetype, and component_type."""

    rrd = tmp_path / "tmp.rrd"

    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(rrd)
        rec.set_time("my_index", sequence=1)
        rec.log("points", rr.Points3D([[1, 2, 3]]))
        rec.log("static_text", rr.TextLog("Hello"), static=True)
        rec.send_property("my_prop", rr.Points2D([[0, 2]]))

    recording = rr.recording.load_recording(rrd)
    schema = recording.schema()

    def names(cols: list) -> list[str]:  # type: ignore[type-arg]
        return sorted(col.name for col in cols)

    # Filter by entity_path
    assert names(schema.columns_for(entity_path="/points")) == ["/points:Points3D:positions"]
    assert names(schema.columns_for(entity_path="/static_text")) == ["/static_text:TextLog:text"]

    # Filter by archetype (fully-qualified)
    assert names(schema.columns_for(archetype="rerun.archetypes.Points3D")) == ["/points:Points3D:positions"]
    assert names(schema.columns_for(archetype="rerun.archetypes.TextLog")) == ["/static_text:TextLog:text"]

    # Filter by archetype (short form)
    assert names(schema.columns_for(archetype="Points3D")) == ["/points:Points3D:positions"]
    assert names(schema.columns_for(archetype="TextLog")) == ["/static_text:TextLog:text"]

    # Filter by archetype (class)
    assert names(schema.columns_for(archetype=rr.Points3D)) == ["/points:Points3D:positions"]
    assert names(schema.columns_for(archetype=rr.TextLog)) == ["/static_text:TextLog:text"]

    # Filter by component_type
    assert names(schema.columns_for(component_type="rerun.components.Text")) == ["/static_text:TextLog:text"]
    assert names(schema.columns_for(component_type="rerun.components.Position3D")) == ["/points:Points3D:positions"]

    # Combined filter
    assert names(schema.columns_for(entity_path="/points", archetype="rerun.archetypes.Points3D")) == [
        "/points:Points3D:positions",
    ]

    # Properties excluded by default
    assert names(schema.columns_for()) == ["/points:Points3D:positions", "/static_text:TextLog:text"]
    assert names(schema.columns_for(include_properties=True)) == [
        "/points:Points3D:positions",
        "/static_text:TextLog:text",
        "property:RecordingInfo:start_time",
        "property:my_prop:Points2D:positions",
    ]

    # Properties included
    assert names(schema.columns_for(include_properties=True, archetype="rerun.archetypes.RecordingInfo")) == [
        "property:RecordingInfo:start_time",
    ]

    # No matches
    assert schema.columns_for(entity_path="/nonexistent") == []


def test_schema_column_names_for(tmp_path: pathlib.Path) -> None:
    """Test Schema.column_names_for() returns filtered column name strings."""

    rrd = tmp_path / "tmp.rrd"

    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(rrd)
        rec.set_time("my_index", sequence=1)
        rec.log("points", rr.Points3D([[1, 2, 3]]))
        rec.log("static_text", rr.TextLog("Hello"), static=True)

    recording = rr.recording.load_recording(rrd)
    schema = recording.schema()

    # Filter by archetype (fully-qualified)
    assert schema.column_names_for(archetype="rerun.archetypes.Points3D") == ["/points:Points3D:positions"]

    # Filter by archetype (short form)
    assert schema.column_names_for(archetype="Points3D") == ["/points:Points3D:positions"]

    # Filter by archetype (class)
    assert schema.column_names_for(archetype=rr.Points3D) == ["/points:Points3D:positions"]

    # Filter by component_type
    assert schema.column_names_for(component_type="rerun.components.Position3D") == ["/points:Points3D:positions"]
    assert schema.column_names_for(component_type="rerun.components.Text") == ["/static_text:TextLog:text"]

    # No matches
    assert schema.column_names_for(entity_path="/nonexistent") == []


def test_load_recording_path_types(tmp_path: pathlib.Path) -> None:
    """Test that load_recording accepts both str and Path."""

    rrd = tmp_path / "tmp.rrd"

    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(rrd)
        rec.log("test", rr.TextLog("Hello"))

    # Test with string path
    recording = rr.recording.load_recording(rrd)
    assert recording is not None

    # Test with Path object
    recording = rr.recording.load_recording(pathlib.Path(tmp_path) / "tmp.rrd")
    assert recording is not None
