from __future__ import annotations

import pytest
import rerun as rr
from rerun.experimental import Chunk


def test_chunk_from_property_batches() -> None:
    points3d = rr.Points3D(positions=[[1.0, 2.0, 3.0]])
    chunk = Chunk.from_property("camera_transform", values=points3d.as_component_batches())

    assert chunk.entity_path == "/__properties/camera_transform"
    assert chunk.is_static
    assert chunk.num_rows == 1
    assert chunk.num_columns == 2  # RowId + component column


def test_chunk_from_property_as_components() -> None:
    chunk = Chunk.from_property("camera_transform", values=rr.Points3D(positions=[[1.0, 2.0, 3.0]]))

    assert chunk.entity_path == "/__properties/camera_transform"
    assert chunk.is_static
    assert chunk.num_rows == 1


def test_chunk_from_property_any_values() -> None:
    # Properties are most commonly basic values, not full Rerun archetypes.
    chunk = Chunk.from_property(
        "episode",
        values=rr.AnyValues(name="kitchen_pick_and_place", success=True, num_attempts=3),
    )

    assert chunk.entity_path == "/__properties/episode"
    assert chunk.is_static
    assert chunk.num_rows == 1
    assert chunk.num_columns == 4  # RowId + one column per value


def test_chunk_from_property_multi_element_values_are_one_row() -> None:
    # All elements land in the cell of a single static row, mirroring `rr.send_property`.
    # A multi-row static chunk would silently drop everything but the last row.
    points3d = rr.Points3D(positions=[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]])
    chunk = Chunk.from_property("calibration_points", values=points3d)

    assert chunk.is_static
    assert chunk.num_rows == 1


def test_chunk_from_property_name_with_slash_is_escaped() -> None:
    # A slash in the name is part of the property name, not a path separator —
    # consistent with `rr.send_property`.
    chunk = Chunk.from_property("a/b", values=rr.AnyValues(value=1))

    assert chunk.entity_path == "/__properties/a\\/b"


def test_chunk_from_property_invalid_values() -> None:
    with pytest.raises(TypeError):
        Chunk.from_property("camera_transform", values=42)  # type: ignore[arg-type]
