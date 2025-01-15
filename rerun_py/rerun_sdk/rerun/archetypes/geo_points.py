# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/geo_points.fbs".

# You can extend this class by creating a "GeoPointsExt" class in "geo_points_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from .geo_points_ext import GeoPointsExt

__all__ = ["GeoPoints"]


@define(str=False, repr=False, init=False)
class GeoPoints(GeoPointsExt, Archetype):
    """
    **Archetype**: Geospatial points with positions expressed in [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees), and optional colors and radii.

    Example
    -------
    ### Log a geospatial point:
    ```python
    import rerun as rr

    rr.init("rerun_example_geo_points", spawn=True)

    rr.log(
        "rerun_hq",
        rr.GeoPoints(
            lat_lon=[59.319221, 18.075631],
            radii=rr.Radius.ui_points(10.0),
            colors=[255, 0, 0],
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/1200w.png">
      <img src="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in geo_points_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            positions=None,
            radii=None,
            colors=None,
            class_ids=None,
        )

    @classmethod
    def _clear(cls) -> GeoPoints:
        """Produce an empty GeoPoints, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        positions: datatypes.DVec2DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> GeoPoints:
        """
        Update only some specific fields of a `GeoPoints`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        positions:
            The [EPSG:4326](https://epsg.io/4326) coordinates for the points (North/East-positive degrees).
        radii:
            Optional radii for the points, effectively turning them into circles.

            *Note*: scene units radiii are interpreted as meters.
        colors:
            Optional colors for the points.

            The colors are interpreted as RGB or RGBA in sRGB gamma-space,
            As either 0-1 floats or 0-255 integers, with separate alpha.
        class_ids:
            Optional class Ids for the points.

            The [`components.ClassId`][rerun.components.ClassId] provides colors if not specified explicitly.

        """

        kwargs = {
            "positions": positions,
            "radii": radii,
            "colors": colors,
            "class_ids": class_ids,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return GeoPoints(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> GeoPoints:
        """Clear all the fields of a `GeoPoints`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            positions=[],
            radii=[],
            colors=[],
            class_ids=[],
        )
        return inst

    positions: components.LatLonBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.LatLonBatch._optional,  # type: ignore[misc]
    )
    # The [EPSG:4326](https://epsg.io/4326) coordinates for the points (North/East-positive degrees).
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusBatch._optional,  # type: ignore[misc]
    )
    # Optional radii for the points, effectively turning them into circles.
    #
    # *Note*: scene units radiii are interpreted as meters.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Optional colors for the points.
    #
    # The colors are interpreted as RGB or RGBA in sRGB gamma-space,
    # As either 0-1 floats or 0-255 integers, with separate alpha.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdBatch._optional,  # type: ignore[misc]
    )
    # Optional class Ids for the points.
    #
    # The [`components.ClassId`][rerun.components.ClassId] provides colors if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
