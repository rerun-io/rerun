from __future__ import annotations

import numpy as np
import pytest
import rerun as rr
from rerun.components import GeoPolygon, GeoPolygonBatch
from rerun.encodings import GeoPolygonDataBatch


def test_variable_ring_counts_arrow_serialization() -> None:
    first = GeoPolygon(
        exterior=[[10, 20], [10, 24], [14, 24], [14, 20]],
        holes=[[[11, 21], [11, 22], [12, 21]]],
    )
    second = GeoPolygon(exterior=np.asarray([[30, 40], [30, 41], [31, 40]], dtype=np.float32))
    array = GeoPolygonBatch([first, second]).as_arrow_array()
    assert array.type == GeoPolygonDataBatch._ARROW_DATATYPE
    assert array.field("ring_lengths").to_pylist() == [[4, 3], [3]]
    assert array.field("lat_lon").to_pylist() == [
        [[10, 20], [10, 24], [14, 24], [14, 20], [11, 21], [11, 22], [12, 21]],
        [[30, 40], [30, 41], [31, 40]],
    ]
    polygons = rr.GeoPolygons([first, second], colors=[[40, 120, 230, 128]])
    assert polygons.polygons is not None
    assert len(polygons.polygons.as_arrow_array()) == 2


def test_empty_batch_and_single_polygon() -> None:
    empty = GeoPolygonBatch([]).as_arrow_array()
    assert len(empty) == 0
    assert empty.type == GeoPolygonDataBatch._ARROW_DATATYPE
    polygon = GeoPolygon(exterior=[[48.8, 2.3], [48.8, 2.4], [48.9, 2.3]])
    assert len(GeoPolygonBatch(polygon).as_arrow_array()) == 1


@pytest.mark.parametrize("ring", [[], [[1, 2]], [[1, 2, 3]] * 3, [[1, 2], [2, 3], [float("nan"), 4]]])
def test_invalid_ring_shape_or_coordinates(ring: list[list[float]]) -> None:
    with pytest.raises(ValueError):
        GeoPolygon(exterior=ring)
