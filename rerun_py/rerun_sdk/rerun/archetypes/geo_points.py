# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/geo_points.fbs".

# You can extend this class by creating a "GeoPointsExt" class in "geo_points_ext.py".

from __future__ import annotations

import numpy as np
import pyarrow as pa
from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from ..error_utils import catch_and_log_exceptions
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
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        positions: datatypes.DVec2DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> GeoPoints:
        """
        Update only some specific fields of a `GeoPoints`.

        Parameters
        ----------
        clear_unset:
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

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "positions": positions,
                "radii": radii,
                "colors": colors,
                "class_ids": class_ids,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> GeoPoints:
        """Clear all the fields of a `GeoPoints`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        positions: datatypes.DVec2DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
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

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                positions=positions,
                radii=radii,
                colors=colors,
                class_ids=class_ids,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {"positions": positions, "radii": radii, "colors": colors, "class_ids": class_ids}
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]
                elem_flat_len = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                if pa.types.is_fixed_size_list(arrow_array.type) and arrow_array.type.list_size == elem_flat_len:
                    # If the product of the last dimensions of the shape are equal to the size of the fixed size list array,
                    # we have `num_rows` single element batches (each element is a fixed sized list).
                    # (This should have been already validated by conversion to the arrow_array)
                    batch_length = 1
                else:
                    batch_length = shape[1] if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    positions: components.LatLonBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.LatLonBatch._converter,  # type: ignore[misc]
    )
    # The [EPSG:4326](https://epsg.io/4326) coordinates for the points (North/East-positive degrees).
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RadiusBatch._converter,  # type: ignore[misc]
    )
    # Optional radii for the points, effectively turning them into circles.
    #
    # *Note*: scene units radiii are interpreted as meters.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # Optional colors for the points.
    #
    # The colors are interpreted as RGB or RGBA in sRGB gamma-space,
    # As either 0-1 floats or 0-255 integers, with separate alpha.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ClassIdBatch._converter,  # type: ignore[misc]
    )
    # Optional class Ids for the points.
    #
    # The [`components.ClassId`][rerun.components.ClassId] provides colors if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
