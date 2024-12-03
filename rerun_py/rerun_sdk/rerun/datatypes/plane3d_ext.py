from __future__ import annotations

from typing import TYPE_CHECKING, Any, Union

import numpy as np
import pyarrow as pa

from .._validators import flat_np_float32_array_from_array_like

if TYPE_CHECKING:
    from . import Plane3D, Plane3DArrayLike, Vec3DLike


class Plane3DExt:
    """Extension for [Plane3D][rerun.datatypes.Plane3D]."""

    # The Y^Z plane with normal = +X.
    YZ: Plane3D = None  # type: ignore[assignment]

    # The Z^X plane with normal = +Y.
    ZX: Plane3D = None  # type: ignore[assignment]

    # The X^Y plane with normal = +Z.
    XY: Plane3D = None  # type: ignore[assignment]

    @staticmethod
    def deferred_patch_class(cls: Any) -> None:
        cls.YZ = cls([1.0, 0.0, 0.0])
        cls.ZX = cls([0.0, 1.0, 0.0])
        cls.XY = cls([0.0, 0.0, 1.0])

    def __init__(self: Any, normal: Vec3DLike, distance: Union[float, int, None] = None) -> None:
        """
        Create a new instance of the Plane3D datatype.

        Does *not* normalize the plane.

        Parameters
        ----------
        normal:
            Normal vector of the plane.
        distance:
            Distance of the plane from the origin.
            Defaults to zero.

        """

        normal_np = flat_np_float32_array_from_array_like(normal, 3)
        if distance is None:
            distance_np = np.array([0.0], dtype=np.float32)
        else:
            distance_np = np.array([distance], dtype=np.float32)

        self.__attrs_init__(xyzd=np.concatenate((normal_np, distance_np)))

    @staticmethod
    def native_to_pa_array_override(data: Plane3DArrayLike, data_type: pa.DataType) -> pa.Array:
        planes = flat_np_float32_array_from_array_like(data, 4)
        return pa.FixedSizeListArray.from_arrays(planes, type=data_type)
