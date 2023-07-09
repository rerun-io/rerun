from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np

if TYPE_CHECKING:
    from .. import Mat3x3Like, Mat4x4Like, Vec3D, Vec4D


def mat3x3_columns_converter(data: Mat3x3Like) -> list[Vec3D]:
    from .. import Mat3x3, Vec3D

    if isinstance(data, Mat3x3):
        return data.columns
    else:
        arr = np.array(data, dtype=np.float32).reshape(3, 3)
        return [Vec3D(datum) for datum in arr.T]


def mat4x4_columns_converter(data: Mat4x4Like) -> list[Vec4D]:
    from .. import Mat4x4, Vec4D

    if isinstance(data, Mat4x4):
        return data.columns
    else:
        arr = np.array(data, dtype=np.float32).reshape(4, 4)
        return [Vec4D(datum) for datum in arr.T]
