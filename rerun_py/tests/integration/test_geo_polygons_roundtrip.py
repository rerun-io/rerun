"""Exercise polygon rings and partial updates through the native recording reader."""

from __future__ import annotations

from typing import TYPE_CHECKING

import rerun as rr
from rerun.chunk import RrdReader
from rerun.components import GeoPolygon

if TYPE_CHECKING:
    from pathlib import Path


def test_geo_polygons_native_recording_roundtrip(tmp_path: Path) -> None:
    exterior = [[10.0, 20.0], [10.0, 24.0], [14.0, 24.0], [14.0, 20.0]]
    hole = [[11.0, 21.0], [11.0, 22.0], [12.0, 21.0]]
    triangle = [[30.0, 40.0], [30.0, 41.0], [31.0, 40.0]]
    first = GeoPolygon(exterior=exterior, holes=[hole])
    second = GeoPolygon(exterior=triangle)
    path = tmp_path / "polygons.rrd"
    with rr.RecordingStream("geo_polygons_roundtrip") as recording:
        recording.save(path)
        recording.set_time("frame", sequence=1)
        recording.log("regions", rr.GeoPolygons([first, second], colors=[[10, 20, 30, 128], [40, 50, 60, 255]]))
        recording.set_time("frame", sequence=2)
        recording.log("regions", rr.GeoPolygons([second], colors=[[30, 180, 80]]))
        recording.set_time("frame", sequence=3)
        recording.log("regions", rr.GeoPolygons.from_fields(colors=[[230, 180, 30]]))
        recording.set_time("frame", sequence=4)
        recording.log("regions", rr.GeoPolygons([]))

    polygon_rows = {}
    color_rows = {}
    for chunk in RrdReader(path).stream():
        if chunk.entity_path != "/regions":
            continue
        batch = chunk.to_record_batch()
        frames = next(
            batch.column(index).to_pylist()
            for index, field in enumerate(batch.schema)
            if (field.metadata or {}).get(b"rerun:index_name") == b"frame"
        )
        for index, field in enumerate(batch.schema):
            component = (field.metadata or {}).get(b"rerun:component_type")
            if component == b"rerun.components.GeoPolygon":
                rows = polygon_rows
            elif component == b"rerun.components.Color":
                rows = color_rows
            else:
                continue
            for frame, value in zip(frames, batch.column(index).to_pylist(), strict=True):
                if value is not None:
                    rows[frame] = value

    first_value = {"lat_lon": exterior + hole, "ring_lengths": [4, 3]}
    second_value = {"lat_lon": triangle, "ring_lengths": [3]}
    assert polygon_rows == {1: [first_value, second_value], 2: [second_value], 4: []}
    assert color_rows == {
        1: [(10 << 24) | (20 << 16) | (30 << 8) | 128, (40 << 24) | (50 << 16) | (60 << 8) | 255],
        2: [(30 << 24) | (180 << 16) | (80 << 8) | 255],
        3: [(230 << 24) | (180 << 16) | (30 << 8) | 255],
    }
