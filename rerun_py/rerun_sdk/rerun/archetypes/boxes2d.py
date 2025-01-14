# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/boxes2d.fbs".

# You can extend this class by creating a "Boxes2DExt" class in "boxes2d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from .boxes2d_ext import Boxes2DExt

__all__ = ["Boxes2D"]


@define(str=False, repr=False, init=False)
class Boxes2D(Boxes2DExt, Archetype):
    """
    **Archetype**: 2D boxes with half-extents and optional center, colors etc.

    Example
    -------
    ### Simple 2D boxes:
    ```python
    import rerun as rr

    rr.init("rerun_example_box2d", spawn=True)

    rr.log("simple", rr.Boxes2D(mins=[-1, -1], sizes=[2, 2]))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/box2d_simple/ac4424f3cf747382867649610cbd749c45b2020b/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/box2d_simple/ac4424f3cf747382867649610cbd749c45b2020b/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/box2d_simple/ac4424f3cf747382867649610cbd749c45b2020b/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/box2d_simple/ac4424f3cf747382867649610cbd749c45b2020b/1200w.png">
      <img src="https://static.rerun.io/box2d_simple/ac4424f3cf747382867649610cbd749c45b2020b/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in boxes2d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            half_sizes=None,  # type: ignore[arg-type]
            centers=None,  # type: ignore[arg-type]
            colors=None,  # type: ignore[arg-type]
            radii=None,  # type: ignore[arg-type]
            labels=None,  # type: ignore[arg-type]
            show_labels=None,  # type: ignore[arg-type]
            draw_order=None,  # type: ignore[arg-type]
            class_ids=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Boxes2D:
        """Produce an empty Boxes2D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        half_sizes: datatypes.Vec2DArrayLike | None = None,
        centers: datatypes.Vec2DArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        draw_order: datatypes.Float32Like | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> Boxes2D:
        """
        Update only some specific fields of a `Boxes2D`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        half_sizes:
            All half-extents that make up the batch of boxes.
        centers:
            Optional center positions of the boxes.
        colors:
            Optional colors for the boxes.
        radii:
            Optional radii for the lines that make up the boxes.
        labels:
            Optional text labels for the boxes.

            If there's a single label present, it will be placed at the center of the entity.
            Otherwise, each instance will have its own label.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        draw_order:
            An optional floating point value that specifies the 2D drawing order.

            Objects with higher values are drawn on top of those with lower values.

            The default for 2D boxes is 10.0.
        class_ids:
            Optional [`components.ClassId`][rerun.components.ClassId]s for the boxes.

            The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.

        """

        kwargs = {
            "half_sizes": half_sizes,
            "centers": centers,
            "colors": colors,
            "radii": radii,
            "labels": labels,
            "show_labels": show_labels,
            "draw_order": draw_order,
            "class_ids": class_ids,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return Boxes2D(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> Boxes2D:
        """Clear all the fields of a `Boxes2D`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            half_sizes=[],  # type: ignore[arg-type]
            centers=[],  # type: ignore[arg-type]
            colors=[],  # type: ignore[arg-type]
            radii=[],  # type: ignore[arg-type]
            labels=[],  # type: ignore[arg-type]
            show_labels=[],  # type: ignore[arg-type]
            draw_order=[],  # type: ignore[arg-type]
            class_ids=[],  # type: ignore[arg-type]
        )
        return inst

    half_sizes: components.HalfSize2DBatch = field(
        metadata={"component": "optional"},
        converter=components.HalfSize2DBatch._optional,  # type: ignore[misc]
    )
    # All half-extents that make up the batch of boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    centers: components.Position2DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Position2DBatch._optional,  # type: ignore[misc]
    )
    # Optional center positions of the boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Optional colors for the boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusBatch._optional,  # type: ignore[misc]
    )
    # Optional radii for the lines that make up the boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextBatch._optional,  # type: ignore[misc]
    )
    # Optional text labels for the boxes.
    #
    # If there's a single label present, it will be placed at the center of the entity.
    # Otherwise, each instance will have its own label.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    show_labels: components.ShowLabelsBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ShowLabelsBatch._optional,  # type: ignore[misc]
    )
    # Optional choice of whether the text labels should be shown by default.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    draw_order: components.DrawOrderBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.DrawOrderBatch._optional,  # type: ignore[misc]
    )
    # An optional floating point value that specifies the 2D drawing order.
    #
    # Objects with higher values are drawn on top of those with lower values.
    #
    # The default for 2D boxes is 10.0.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdBatch._optional,  # type: ignore[misc]
    )
    # Optional [`components.ClassId`][rerun.components.ClassId]s for the boxes.
    #
    # The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
