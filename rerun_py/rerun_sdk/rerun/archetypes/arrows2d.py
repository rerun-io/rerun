# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/arrows2d.fbs".

# You can extend this class by creating a "Arrows2DExt" class in "arrows2d_ext.py".

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
from .arrows2d_ext import Arrows2DExt

__all__ = ["Arrows2D"]


@define(str=False, repr=False, init=False)
class Arrows2D(Arrows2DExt, Archetype):
    """
    **Archetype**: 2D arrows with optional colors, radii, labels, etc.

    Example
    -------
    ### Simple batch of 2D arrows:
    ```python
    import rerun as rr

    rr.init("rerun_example_arrow2d", spawn=True)

    rr.log(
        "arrows",
        rr.Arrows2D(
            origins=[[0.25, 0.0], [0.25, 0.0], [-0.1, -0.1]],
            vectors=[[1.0, 0.0], [0.0, -1.0], [-0.7, 0.7]],
            colors=[[255, 0, 0], [0, 255, 0], [127, 0, 255]],
            labels=["right", "up", "left-down"],
            radii=0.025,
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/arrow2d_simple/59f044ccc03f7bc66ee802288f75706618b29a6e/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/arrow2d_simple/59f044ccc03f7bc66ee802288f75706618b29a6e/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/arrow2d_simple/59f044ccc03f7bc66ee802288f75706618b29a6e/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/arrow2d_simple/59f044ccc03f7bc66ee802288f75706618b29a6e/1200w.png">
      <img src="https://static.rerun.io/arrow2d_simple/59f044ccc03f7bc66ee802288f75706618b29a6e/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in arrows2d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            vectors=None,
            origins=None,
            radii=None,
            colors=None,
            labels=None,
            show_labels=None,
            draw_order=None,
            class_ids=None,
        )

    @classmethod
    def _clear(cls) -> Arrows2D:
        """Produce an empty Arrows2D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        vectors: datatypes.Vec2DArrayLike | None = None,
        origins: datatypes.Vec2DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        draw_order: datatypes.Float32Like | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> Arrows2D:
        """
        Update only some specific fields of a `Arrows2D`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        vectors:
            All the vectors for each arrow in the batch.
        origins:
            All the origin (base) positions for each arrow in the batch.

            If no origins are set, (0, 0) is used as the origin for each arrow.
        radii:
            Optional radii for the arrows.

            The shaft is rendered as a line with `radius = 0.5 * radius`.
            The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
        colors:
            Optional colors for the points.
        labels:
            Optional text labels for the arrows.

            If there's a single label present, it will be placed at the center of the entity.
            Otherwise, each instance will have its own label.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        draw_order:
            An optional floating point value that specifies the 2D drawing order.

            Objects with higher values are drawn on top of those with lower values.
        class_ids:
            Optional class Ids for the points.

            The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "vectors": vectors,
                "origins": origins,
                "radii": radii,
                "colors": colors,
                "labels": labels,
                "show_labels": show_labels,
                "draw_order": draw_order,
                "class_ids": class_ids,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> Arrows2D:
        """Clear all the fields of a `Arrows2D`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        vectors: datatypes.Vec2DArrayLike | None = None,
        origins: datatypes.Vec2DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolArrayLike | None = None,
        draw_order: datatypes.Float32ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        vectors:
            All the vectors for each arrow in the batch.
        origins:
            All the origin (base) positions for each arrow in the batch.

            If no origins are set, (0, 0) is used as the origin for each arrow.
        radii:
            Optional radii for the arrows.

            The shaft is rendered as a line with `radius = 0.5 * radius`.
            The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
        colors:
            Optional colors for the points.
        labels:
            Optional text labels for the arrows.

            If there's a single label present, it will be placed at the center of the entity.
            Otherwise, each instance will have its own label.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        draw_order:
            An optional floating point value that specifies the 2D drawing order.

            Objects with higher values are drawn on top of those with lower values.
        class_ids:
            Optional class Ids for the points.

            The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                vectors=vectors,
                origins=origins,
                radii=radii,
                colors=colors,
                labels=labels,
                show_labels=show_labels,
                draw_order=draw_order,
                class_ids=class_ids,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "vectors": vectors,
            "origins": origins,
            "radii": radii,
            "colors": colors,
            "labels": labels,
            "show_labels": show_labels,
            "draw_order": draw_order,
            "class_ids": class_ids,
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

    vectors: components.Vector2DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.Vector2DBatch._converter,  # type: ignore[misc]
    )
    # All the vectors for each arrow in the batch.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    origins: components.Position2DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.Position2DBatch._converter,  # type: ignore[misc]
    )
    # All the origin (base) positions for each arrow in the batch.
    #
    # If no origins are set, (0, 0) is used as the origin for each arrow.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RadiusBatch._converter,  # type: ignore[misc]
    )
    # Optional radii for the arrows.
    #
    # The shaft is rendered as a line with `radius = 0.5 * radius`.
    # The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # Optional colors for the points.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TextBatch._converter,  # type: ignore[misc]
    )
    # Optional text labels for the arrows.
    #
    # If there's a single label present, it will be placed at the center of the entity.
    # Otherwise, each instance will have its own label.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    show_labels: components.ShowLabelsBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ShowLabelsBatch._converter,  # type: ignore[misc]
    )
    # Optional choice of whether the text labels should be shown by default.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    draw_order: components.DrawOrderBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.DrawOrderBatch._converter,  # type: ignore[misc]
    )
    # An optional floating point value that specifies the 2D drawing order.
    #
    # Objects with higher values are drawn on top of those with lower values.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ClassIdBatch._converter,  # type: ignore[misc]
    )
    # Optional class Ids for the points.
    #
    # The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
