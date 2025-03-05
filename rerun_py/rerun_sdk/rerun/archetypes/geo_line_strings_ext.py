from __future__ import annotations

from typing import Any

from .. import components, datatypes
from ..error_utils import catch_and_log_exceptions


class GeoLineStringsExt:
    """Extension for [GeoLineStrings][rerun.archetypes.GeoLineStrings]."""

    # TODO(ab): the purpose of this override is to rename the required parameter and make it keyword-only. Should be codegen-able?
    def __init__(
        self: Any,
        *,
        lat_lon: components.GeoLineStringArrayLike,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the GeoLineStrings archetype.

        Parameters
        ----------
        lat_lon:
            The line strings, expressed in [EPSG:4326](https://epsg.io/4326) coordinates (North/East-positive degrees).
        radii:
            Optional radii for the line strings.
        colors:
            Optional colors for the linestrings.

            The colors are interpreted as RGB or RGBA in sRGB gamma-space,
            As either 0-1 floats or 0-255 integers, with separate alpha.

        """

        # You can define your own __init__ function as a member of GeoLineStringsExt in geo_line_strings_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(line_strings=lat_lon, radii=radii, colors=colors)
            return
        self.__attrs_clear__()
