from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import components, datatypes


class Cones3DExt:
    """Extension for [Cones3D][rerun.archetypes.Cones3D]."""

    def __init__(
        self: Any,
        *,
        lengths: datatypes.Float32ArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        centers: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        line_radii: datatypes.Float32ArrayLike | None = None,
        fill_mode: components.FillModeLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> None:
        """Create a new instance of the Cones3D archetype."""

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                lengths=lengths,
                radii=radii,
                centers=centers,
                rotation_axis_angles=rotation_axis_angles,
                quaternions=quaternions,
                colors=colors,
                line_radii=line_radii,
                fill_mode=fill_mode,
                labels=labels,
                show_labels=show_labels,
                class_ids=class_ids,
            )
            return

        self.__attrs_clear__()
