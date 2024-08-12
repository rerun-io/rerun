from __future__ import annotations

from typing import TYPE_CHECKING, Any, Union

if TYPE_CHECKING:
    from rerun.datatypes import Float32Like, Vec3DLike


class PoseScale3DExt:
    """Extension for [PoseScale3D][rerun.components.PoseScale3D]."""

    def __init__(
        self: Any,
        uniform_or_per_axis: Union[Vec3DLike, Float32Like] = True,
    ):
        """
        3D scaling factor.

        A scale of 1.0 means no scaling.
        A scale of 2.0 means doubling the size.
        Each component scales along the corresponding axis.

        Parameters
        ----------
        uniform_or_per_axis:
            If a single value is given, it is applied the same to all three axis (uniform scaling).

        """
        if not hasattr(uniform_or_per_axis, "__len__") or len(uniform_or_per_axis) == 1:  # type: ignore[arg-type]
            self.__attrs_init__([uniform_or_per_axis, uniform_or_per_axis, uniform_or_per_axis])
        else:
            self.__attrs_init__(uniform_or_per_axis)
