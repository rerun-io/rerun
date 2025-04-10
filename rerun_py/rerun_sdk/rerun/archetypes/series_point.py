# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/series_point.fbs".

# You can extend this class by creating a "SeriesPointExt" class in "series_point_ext.py".

from __future__ import annotations

from typing import Any

import numpy as np
import pyarrow as pa
from attrs import define, field
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from ..error_utils import catch_and_log_exceptions

__all__ = ["SeriesPoint"]


@deprecated("""since 0.23.0: Use `SeriesPoints` instead.""")
@define(str=False, repr=False, init=False)
class SeriesPoint(Archetype):
    """
    **Archetype**: Define the style properties for a point series in a chart.

    This archetype only provides styling information and should be logged as static
    when possible. The underlying data needs to be logged to the same entity-path using
    [`archetypes.Scalars`][rerun.archetypes.Scalars].

    ⚠️ **Deprecated since 0.23.0**: Use `SeriesPoints` instead.

    Example
    -------
    ### Point series:
    ```python
    from math import cos, sin, tau

    import rerun as rr

    rr.init("rerun_example_series_point_style", spawn=True)

    # Set up plot styling:
    # They are logged as static as they don't change over time and apply to all timelines.
    # Log two point series under a shared root so that they show in the same plot by default.
    rr.log(
        "trig/sin",
        rr.SeriesPoints(
            colors=[255, 0, 0],
            names="sin(0.01t)",
            markers="circle",
            marker_sizes=4,
        ),
        static=True,
    )
    rr.log(
        "trig/cos",
        rr.SeriesPoints(
            colors=[0, 255, 0],
            names="cos(0.01t)",
            markers="cross",
            marker_sizes=2,
        ),
        static=True,
    )


    # Log the data on a timeline called "step".
    for t in range(int(tau * 2 * 10.0)):
        rr.set_time("step", sequence=t)

        rr.log("trig/sin", rr.Scalars(sin(float(t) / 10.0)))
        rr.log("trig/cos", rr.Scalars(cos(float(t) / 10.0)))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/1200w.png">
      <img src="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(
        self: Any,
        *,
        color: datatypes.Rgba32Like | None = None,
        marker: components.MarkerShapeLike | None = None,
        name: datatypes.Utf8Like | None = None,
        visible_series: datatypes.BoolArrayLike | None = None,
        marker_size: datatypes.Float32Like | None = None,
    ) -> None:
        """
        Create a new instance of the SeriesPoint archetype.

        Parameters
        ----------
        color:
            Color for the corresponding series.
        marker:
            What shape to use to represent the point
        name:
            Display name of the series.

            Used in the legend.
        visible_series:
            Which point series are visible.

            If not set, all point series on this entity are visible.
            Unlike with the regular visibility property of the entire entity, any series that is hidden
            via this property will still be visible in the legend.
        marker_size:
            Size of the marker.

        """

        # You can define your own __init__ function as a member of SeriesPointExt in series_point_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                color=color, marker=marker, name=name, visible_series=visible_series, marker_size=marker_size
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            color=None,
            marker=None,
            name=None,
            visible_series=None,
            marker_size=None,
        )

    @classmethod
    @deprecated("""since 0.23.0: Use `SeriesPoints` instead.""")
    def _clear(cls) -> SeriesPoint:
        """Produce an empty SeriesPoint, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    @deprecated("""since 0.23.0: Use `SeriesPoints` instead.""")
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        color: datatypes.Rgba32Like | None = None,
        marker: components.MarkerShapeLike | None = None,
        name: datatypes.Utf8Like | None = None,
        visible_series: datatypes.BoolArrayLike | None = None,
        marker_size: datatypes.Float32Like | None = None,
    ) -> SeriesPoint:
        """
        Update only some specific fields of a `SeriesPoint`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        color:
            Color for the corresponding series.
        marker:
            What shape to use to represent the point
        name:
            Display name of the series.

            Used in the legend.
        visible_series:
            Which point series are visible.

            If not set, all point series on this entity are visible.
            Unlike with the regular visibility property of the entire entity, any series that is hidden
            via this property will still be visible in the legend.
        marker_size:
            Size of the marker.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "color": color,
                "marker": marker,
                "name": name,
                "visible_series": visible_series,
                "marker_size": marker_size,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> SeriesPoint:
        """Clear all the fields of a `SeriesPoint`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    @deprecated("""since 0.23.0: Use `SeriesPoints` instead.""")
    def columns(
        cls,
        *,
        color: datatypes.Rgba32ArrayLike | None = None,
        marker: components.MarkerShapeArrayLike | None = None,
        name: datatypes.Utf8ArrayLike | None = None,
        visible_series: datatypes.BoolArrayLike | None = None,
        marker_size: datatypes.Float32ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        color:
            Color for the corresponding series.
        marker:
            What shape to use to represent the point
        name:
            Display name of the series.

            Used in the legend.
        visible_series:
            Which point series are visible.

            If not set, all point series on this entity are visible.
            Unlike with the regular visibility property of the entire entity, any series that is hidden
            via this property will still be visible in the legend.
        marker_size:
            Size of the marker.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                color=color,
                marker=marker,
                name=name,
                visible_series=visible_series,
                marker_size=marker_size,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "color": color,
            "marker": marker,
            "name": name,
            "visible_series": visible_series,
            "marker_size": marker_size,
        }
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
                    batch_length = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    color: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # Color for the corresponding series.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    marker: components.MarkerShapeBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.MarkerShapeBatch._converter,  # type: ignore[misc]
    )
    # What shape to use to represent the point
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    name: components.NameBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.NameBatch._converter,  # type: ignore[misc]
    )
    # Display name of the series.
    #
    # Used in the legend.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    visible_series: components.SeriesVisibleBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.SeriesVisibleBatch._converter,  # type: ignore[misc]
    )
    # Which point series are visible.
    #
    # If not set, all point series on this entity are visible.
    # Unlike with the regular visibility property of the entire entity, any series that is hidden
    # via this property will still be visible in the legend.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    marker_size: components.MarkerSizeBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.MarkerSizeBatch._converter,  # type: ignore[misc]
    )
    # Size of the marker.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
