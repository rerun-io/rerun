from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np
import numpy.typing as npt
import pyarrow as pa

if TYPE_CHECKING:
    from . import Quaternion, QuaternionArrayLike


NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class QuaternionExt:
    def __init__(self: Any, *, xyzw: npt.ArrayLike) -> None:
        self.__attrs_init__(xyzw=xyzw)

    @staticmethod
    def identity() -> Quaternion:
        from . import Quaternion

        return Quaternion(xyzw=np.array([0, 0, 0, 1], dtype=np.float32))

    @staticmethod
    def native_to_pa_array_override(data: QuaternionArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Quaternion

        if isinstance(data, Quaternion):
            data = [data]

        quaternions = np.asarray([q.xyzw for q in data], dtype=np.float32).reshape((-1,))
        return pa.FixedSizeListArray.from_arrays(quaternions, type=data_type)
