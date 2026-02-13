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
