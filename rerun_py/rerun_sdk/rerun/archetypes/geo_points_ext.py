from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np

from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import datatypes

NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class GeoPointsExt:
    """Extension for [GeoPoints][rerun.archetypes.GeoPoints]."""

    # TODO(ab): the purpose of this override is to rename the required parameter and make it keyword-only. Should be codegen-able?
    def __init__(
        self: Any,
        *,
        lat_lon: datatypes.DVec2DArrayLike,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the GeoPoints archetype.

        Parameters
        ----------
        lat_lon:
            The [EPSG:4326](https://epsg.io/4326) coordinates for the points (North/East-positive degrees).
        radii:
            Optional radii for the points, effectively turning them into circles.
        colors:
            Optional colors for the points.

            The colors are interpreted as RGB or RGBA in sRGB gamma-space,
            As either 0-1 floats or 0-255 integers, with separate alpha.

        """

        # You can define your own __init__ function as a member of GeoPointsExt in geo_points_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(positions=lat_lon, radii=radii, colors=colors)
            return
        self.__attrs_clear__()
