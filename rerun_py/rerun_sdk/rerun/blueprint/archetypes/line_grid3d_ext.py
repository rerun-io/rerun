from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ...error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from ... import datatypes


class LineGrid3DExt:
    """Extension for [LineGrid3D][rerun.blueprint.archetypes.LineGrid3D]."""

    def __init__(
        self: Any,
        visible: datatypes.BoolLike | None = None,
        *,
        spacing: datatypes.Float32Like | None = None,
        plane: datatypes.Plane3DLike | None = None,
        stroke_width: datatypes.Float32Like | None = None,
        color: datatypes.Rgba32Like | None = None,
    ) -> None:
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
