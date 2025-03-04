from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from .._validators import flat_np_float32_array_from_array_like

if TYPE_CHECKING:
    from . import Quaternion, QuaternionArrayLike


NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class QuaternionExt:
    """Extension for [Quaternion][rerun.datatypes.Quaternion]."""

    def __init__(self: Any, *, xyzw: npt.ArrayLike) -> None:
        self.__attrs_init__(xyzw=xyzw)

    @staticmethod
    def identity() -> Quaternion:
        from . import Quaternion

        return Quaternion(xyzw=np.array([0, 0, 0, 1], dtype=np.float32))

    @staticmethod
    def invalid() -> Quaternion:
        from . import Quaternion

        return Quaternion(xyzw=np.array([0, 0, 0, 0], dtype=np.float32))

    @staticmethod
    def native_to_pa_array_override(data: QuaternionArrayLike, data_type: pa.DataType) -> pa.Array:
        quaternions = flat_np_float32_array_from_array_like(data, 4)
        return pa.FixedSizeListArray.from_arrays(quaternions, type=data_type)
