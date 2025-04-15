# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/geo_line_strings.fbs".

# You can extend this class by creating a "GeoLineStringsExt" class in "geo_line_strings_ext.py".

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
from .geo_line_strings_ext import GeoLineStringsExt

__all__ = ["GeoLineStrings"]


@define(str=False, repr=False, init=False)
class GeoLineStrings(GeoLineStringsExt, Archetype):
    """
    **Archetype**: Geospatial line strings with positions expressed in [EPSG:4326](https://epsg.io/4326) altitude and longitude (North/East-positive degrees), and optional colors and radii.

    Also known as "line strips" or "polylines".

    Example
    -------
    ### Log a geospatial line string:
    ```python
    import rerun as rr

    rr.init("rerun_example_geo_line_strings", spawn=True)

    rr.log(
        "colorado",
        rr.GeoLineStrings(
            lat_lon=[
                [41.0000, -109.0452],
                [41.0000, -102.0415],
                [36.9931, -102.0415],
                [36.9931, -109.0452],
                [41.0000, -109.0452],
            ],
            radii=rr.Radius.ui_points(2.0),
            colors=[0, 0, 255],
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/geo_line_strings_simple/5669983eb10906ace303755b5b5039cad75b917f/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/geo_line_strings_simple/5669983eb10906ace303755b5b5039cad75b917f/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/geo_line_strings_simple/5669983eb10906ace303755b5b5039cad75b917f/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/geo_line_strings_simple/5669983eb10906ace303755b5b5039cad75b917f/1200w.png">
      <img src="https://static.rerun.io/geo_line_strings_simple/5669983eb10906ace303755b5b5039cad75b917f/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in geo_line_strings_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            line_strings=None,
            radii=None,
            colors=None,
        )

    @classmethod
    def _clear(cls) -> GeoLineStrings:
        """Produce an empty GeoLineStrings, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        line_strings: components.GeoLineStringArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
    ) -> GeoLineStrings:
        """
        Update only some specific fields of a `GeoLineStrings`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        line_strings:
            The line strings, expressed in [EPSG:4326](https://epsg.io/4326) coordinates (North/East-positive degrees).
        radii:
            Optional radii for the line strings.

            *Note*: scene units radiii are interpreted as meters. Currently, the display scale only considers the latitude of
            the first vertex of each line string (see [this issue](https://github.com/rerun-io/rerun/issues/8013)).
        colors:
            Optional colors for the line strings.

            The colors are interpreted as RGB or RGBA in sRGB gamma-space,
            As either 0-1 floats or 0-255 integers, with separate alpha.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "line_strings": line_strings,
                "radii": radii,
                "colors": colors,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> GeoLineStrings:
        """Clear all the fields of a `GeoLineStrings`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        line_strings: components.GeoLineStringArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        line_strings:
            The line strings, expressed in [EPSG:4326](https://epsg.io/4326) coordinates (North/East-positive degrees).
        radii:
            Optional radii for the line strings.

            *Note*: scene units radiii are interpreted as meters. Currently, the display scale only considers the latitude of
            the first vertex of each line string (see [this issue](https://github.com/rerun-io/rerun/issues/8013)).
        colors:
            Optional colors for the line strings.

            The colors are interpreted as RGB or RGBA in sRGB gamma-space,
            As either 0-1 floats or 0-255 integers, with separate alpha.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                line_strings=line_strings,
                radii=radii,
                colors=colors,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {"line_strings": line_strings, "radii": radii, "colors": colors}
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]

                if pa.types.is_fixed_size_list(arrow_array.type) and len(shape) <= 2:
                    # If shape length is 2 or less, we have `num_rows` single element batches (each element is a fixed sized list).
                    # `shape[1]` should be the length of the fixed sized list.
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

    line_strings: components.GeoLineStringBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.GeoLineStringBatch._converter,  # type: ignore[misc]
    )
    # The line strings, expressed in [EPSG:4326](https://epsg.io/4326) coordinates (North/East-positive degrees).
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RadiusBatch._converter,  # type: ignore[misc]
    )
    # Optional radii for the line strings.
    #
    # *Note*: scene units radiii are interpreted as meters. Currently, the display scale only considers the latitude of
    # the first vertex of each line string (see [this issue](https://github.com/rerun-io/rerun/issues/8013)).
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # Optional colors for the line strings.
    #
    # The colors are interpreted as RGB or RGBA in sRGB gamma-space,
    # As either 0-1 floats or 0-255 integers, with separate alpha.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
