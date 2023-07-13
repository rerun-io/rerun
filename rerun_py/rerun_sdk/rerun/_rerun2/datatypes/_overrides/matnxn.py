from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import numpy.typing as npt

if TYPE_CHECKING:
    from .. import Mat3x3Like, Mat4x4Like


def mat3x3_coeffs_converter(data: Mat3x3Like) -> npt.NDArray[np.float32]:
    from .. import Mat3x3

    if isinstance(data, Mat3x3):
        return data.coeffs
    else:
        arr = np.array(data, dtype=np.float32).reshape(3, 3)
        return arr.flatten("F")


def mat4x4_coeffs_converter(data: Mat4x4Like) -> npt.NDArray[np.float32]:
    from .. import Mat4x4

    if isinstance(data, Mat4x4):
        return data.coeffs
    else:
        arr = np.array(data, dtype=np.float32).reshape(4, 4)
        return arr.flatten("F")
