# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/line_grid3d.fbs".

# You can extend this class by creating a "LineGrid3DExt" class in "line_grid3d_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import components, datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["LineGrid3D"]


@define(str=False, repr=False, init=False)
class LineGrid3D(Archetype):
    """**Archetype**: Configuration for the 3D line grid."""

    def __init__(
        self: Any,
        *,
        visible: datatypes.BoolLike | None = None,
        spacing: datatypes.Float32Like | None = None,
        plane: datatypes.Plane3DLike | None = None,
        stroke_width: datatypes.Float32Like | None = None,
        color: datatypes.Rgba32Like | None = None,
    ):
        """
        Create a new instance of the LineGrid3D archetype.

        Parameters
        ----------
        visible:
            Whether the grid is visible.

            Defaults to true.
        spacing:
            Space between grid lines spacing of one line to the next in scene units.

            As you zoom out, successively only every tenth line is shown.
            This controls the closest zoom level.
        plane:
            In what plane the grid is drawn.

            Defaults to whatever plane is determined as the plane at zero units up/down as defined by [`components.ViewCoordinates`][rerun.components.ViewCoordinates] if present.
        stroke_width:
            How thick the lines should be in ui units.

            Default is 1.0 ui unit.
        color:
            Color used for the grid.

            Transparency via alpha channel is supported.
            Defaults to a slightly transparent light gray.

        """

        # You can define your own __init__ function as a member of LineGrid3DExt in line_grid3d_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(visible=visible, spacing=spacing, plane=plane, stroke_width=stroke_width, color=color)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            visible=None,  # type: ignore[arg-type]
            spacing=None,  # type: ignore[arg-type]
            plane=None,  # type: ignore[arg-type]
            stroke_width=None,  # type: ignore[arg-type]
            color=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> LineGrid3D:
        """Produce an empty LineGrid3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        visible: datatypes.BoolLike | None = None,
        spacing: datatypes.Float32Like | None = None,
        plane: datatypes.Plane3DLike | None = None,
        stroke_width: datatypes.Float32Like | None = None,
        color: datatypes.Rgba32Like | None = None,
    ) -> LineGrid3D:
        """
        Update only some specific fields of a `LineGrid3D`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        visible:
            Whether the grid is visible.

            Defaults to true.
        spacing:
            Space between grid lines spacing of one line to the next in scene units.

            As you zoom out, successively only every tenth line is shown.
            This controls the closest zoom level.
        plane:
            In what plane the grid is drawn.

            Defaults to whatever plane is determined as the plane at zero units up/down as defined by [`components.ViewCoordinates`][rerun.components.ViewCoordinates] if present.
        stroke_width:
            How thick the lines should be in ui units.

            Default is 1.0 ui unit.
        color:
            Color used for the grid.

            Transparency via alpha channel is supported.
            Defaults to a slightly transparent light gray.

        """

        kwargs = {
            "visible": visible,
            "spacing": spacing,
            "plane": plane,
            "stroke_width": stroke_width,
            "color": color,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return LineGrid3D(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> LineGrid3D:
        """Clear all the fields of a `LineGrid3D`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            visible=[],  # type: ignore[arg-type]
            spacing=[],  # type: ignore[arg-type]
            plane=[],  # type: ignore[arg-type]
            stroke_width=[],  # type: ignore[arg-type]
            color=[],  # type: ignore[arg-type]
        )
        return inst

    visible: blueprint_components.VisibleBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.VisibleBatch._optional,  # type: ignore[misc]
    )
    # Whether the grid is visible.
    #
    # Defaults to true.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    spacing: blueprint_components.GridSpacingBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.GridSpacingBatch._optional,  # type: ignore[misc]
    )
    # Space between grid lines spacing of one line to the next in scene units.
    #
    # As you zoom out, successively only every tenth line is shown.
    # This controls the closest zoom level.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    plane: components.Plane3DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Plane3DBatch._optional,  # type: ignore[misc]
    )
    # In what plane the grid is drawn.
    #
    # Defaults to whatever plane is determined as the plane at zero units up/down as defined by [`components.ViewCoordinates`][rerun.components.ViewCoordinates] if present.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    stroke_width: components.StrokeWidthBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.StrokeWidthBatch._optional,  # type: ignore[misc]
    )
    # How thick the lines should be in ui units.
    #
    # Default is 1.0 ui unit.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    color: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Color used for the grid.
    #
    # Transparency via alpha channel is supported.
    # Defaults to a slightly transparent light gray.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
