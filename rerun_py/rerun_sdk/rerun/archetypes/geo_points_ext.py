from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import numpy as np

from .. import datatypes
from .._converters import to_np_float64

if TYPE_CHECKING:
    from .. import GeoPoints

NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class GeoPointsExt:
    """Extension for [GeoPoints][rerun.archetypes.GeoPoints]."""

    @staticmethod
    def from_lat_lon(
        positions: datatypes.DVec2DArrayLike,
        *,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
    ) -> GeoPoints:
        """
        Create a new instance of the GeoPoints archetype using latitudes and longitudes, in that order.

        *Note*: this is how Rerun natively stores geospatial data.

        Parameters
        ----------
        positions:
            The [EPSG:4326](https://epsg.io/4326) latitudes and longitudes (in that order) coordinates for the points (North/East-positive degrees).
        radii:
            Optional radii for the points, effectively turning them into circles.
        colors:
            Optional colors for the points.

            The colors are interpreted as RGB or RGBA in sRGB gamma-space,
            As either 0-1 floats or 0-255 integers, with separate alpha.

        """

        from .. import GeoPoints

        return GeoPoints(positions, radii=radii, colors=colors)

    @staticmethod
    def from_lon_lat(
        positions: datatypes.DVec2DArrayLike,
        *,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
    ) -> GeoPoints:
        """
        Create a new instance of the GeoPoints archetype using longitude and latitudes, in that order.

        *Note*: Rerun stores latitude first, so this method converts the input to a Numpy array and swaps the
        coordinates first.

        Parameters
        ----------
        positions:
            The [EPSG:4326](https://epsg.io/4326) latitudes and longitudes (in that order) coordinates for the points (North/East-positive degrees).
        radii:
            Optional radii for the points, effectively turning them into circles.
        colors:
            Optional colors for the points.

            The colors are interpreted as RGB or RGBA in sRGB gamma-space,
            As either 0-1 floats or 0-255 integers, with separate alpha.

        """

        from .. import GeoPoints
        from ..datatypes import DVec2D

        if isinstance(positions, Sequence):
            flipped_pos = np.array([np.array(p.xy) if isinstance(p, DVec2D) else p for p in positions])
        elif isinstance(positions, DVec2D):
            flipped_pos = np.array(positions.xy)
        else:
            flipped_pos = to_np_float64(positions)

        return GeoPoints(np.fliplr(flipped_pos), radii=radii, colors=colors)
