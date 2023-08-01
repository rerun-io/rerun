from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import numpy as np

if TYPE_CHECKING:
    from .. import Scale3DLike, Vec3D


def scale3d_inner_converter(data: Scale3DLike) -> Vec3D | float:
    from .. import Scale3D, Vec3D

    if isinstance(data, Vec3D):
        return data
    elif isinstance(data, Scale3D):
        return data.inner
    elif isinstance(data, float):
        return float(data)
    else:
        return Vec3D(np.array(data))
