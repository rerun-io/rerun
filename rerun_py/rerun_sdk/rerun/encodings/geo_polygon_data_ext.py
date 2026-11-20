from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import GeoPolygonDataArrayLike


class GeoPolygonDataExt:
    """Extension for [GeoPolygonData][rerun.encodings.GeoPolygonData]."""

    @staticmethod
    def native_to_pa_array_override(data: GeoPolygonDataArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import DVec2DBatch, GeoPolygonData

        if isinstance(data, GeoPolygonData):
            polygons = [data]
        elif isinstance(data, Sequence):
            polygons = list(data)
        else:
            raise TypeError("Expected a GeoPolygonData or a sequence of polygons.")

        vertex_offsets = [0]
        ring_offsets = [0]
        vertex_arrays = []
        ring_arrays = []
        for polygon in polygons:
            if not isinstance(polygon, GeoPolygonData):
                raise TypeError("Expected a sequence of GeoPolygonData objects.")
            vertex_arrays.append(DVec2DBatch(polygon.lat_lon).as_arrow_array())
            ring_arrays.append(pa.array(polygon.ring_lengths, type=pa.uint64()))
            vertex_offsets.append(vertex_offsets[-1] + len(vertex_arrays[-1]))
            ring_offsets.append(ring_offsets[-1] + len(ring_arrays[-1]))
        vertices = pa.concat_arrays(vertex_arrays) if vertex_arrays else DVec2DBatch([]).as_arrow_array()
        lengths = pa.concat_arrays(ring_arrays) if ring_arrays else pa.array([], type=pa.uint64())
        return pa.StructArray.from_arrays(
            [
                pa.ListArray.from_arrays(vertex_offsets, vertices, type=data_type.field("lat_lon").type),
                pa.ListArray.from_arrays(ring_offsets, lengths, type=data_type.field("ring_lengths").type),
            ],
            fields=list(data_type),
        )
