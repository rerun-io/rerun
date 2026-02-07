from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import datatypes


class Points2DExt:
    """Extension for [Points2D][rerun.archetypes.Points2D]."""

    def __init__(
        self: Any,
        positions: datatypes.Vec2DArrayLike,
        *,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        draw_order: datatypes.Float32ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        keypoint_ids: datatypes.KeypointIdArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Points2D archetype.

        Parameters
        ----------
        positions:
             All the 2D positions at which the point cloud shows points.
        radii:
             Optional radii for the points, effectively turning them into circles.
        colors:
             Optional colors for the points.

             The colors are interpreted as RGB or RGBA in sRGB gamma-space,
             As either 0-1 floats or 0-255 integers, with separate alpha.
        labels:
             Optional text labels for the points.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        draw_order:
             An optional floating point value that specifies the 2D drawing order.
             Objects with higher values are drawn on top of those with lower values.
        class_ids:
             Optional class Ids for the points.

             The class ID provides colors and labels if not specified explicitly.
        keypoint_ids:
             Optional keypoint IDs for the points, identifying them within a class.

             If keypoint IDs are passed in but no class IDs were specified, the class ID will
             default to 0.
             This is useful to identify points within a single classification (which is identified
             with `class_id`).
             E.g. the classification might be 'Person' and the keypoints refer to joints on a
             detected skeleton.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if class_ids is None and keypoint_ids is not None:
                class_ids = 0

            self.__attrs_init__(
                positions=positions,
                radii=radii,
                colors=colors,
                labels=labels,
                show_labels=show_labels,
                draw_order=draw_order,
                class_ids=class_ids,
                keypoint_ids=keypoint_ids,
            )
            return
        self.__attrs_clear__()
