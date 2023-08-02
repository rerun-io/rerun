from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from .. import Vec2DArrayLike, Vec3DArrayLike, Vec4DArrayLike


NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


def vec2d_native_to_pa_array(data: Vec2DArrayLike, data_type: pa.DataType) -> pa.Array:
    # TODO(ab): get rid of this once we drop support for Python 3.8. Make sure to pin numpy>=1.25.
    if NUMPY_VERSION < (1, 25):
        # Older numpy doesn't seem to support `data` in the form of [Point3D(1, 2), Point3D(3, 4)]
        # this happens for python 3.8 (1.25 supports 3.9+)
        from .. import Vec2D

        if isinstance(data, Sequence):
            data = [np.array(p.xy) if isinstance(p, Vec2D) else p for p in data]  # type: ignore[assignment]

    points = np.asarray(data, dtype=np.float32).reshape((-1,))
    return pa.FixedSizeListArray.from_arrays(points, type=data_type)


def vec3d_native_to_pa_array(data: Vec3DArrayLike, data_type: pa.DataType) -> pa.Array:
    # TODO(ab): get rid of this once we drop support for Python 3.8. Make sure to pin numpy>=1.25.
    if NUMPY_VERSION < (1, 25):
        # Older numpy doesn't seem to support `data` in the form of [Point3D(1, 2), Point3D(3, 4)]
        # this happens for python 3.8 (1.25 supports 3.9+)
        from .. import Vec3D

        if isinstance(data, Sequence):
            data = [np.array(p.xyz) if isinstance(p, Vec3D) else p for p in data]  # type: ignore[assignment]

    points = np.asarray(data, dtype=np.float32).reshape((-1,))
    return pa.FixedSizeListArray.from_arrays(points, type=data_type)


def vec4d_native_to_pa_array(data: Vec4DArrayLike, data_type: pa.DataType) -> pa.Array:
    # TODO(ab): get rid of this once we drop support for Python 3.8. Make sure to pin numpy>=1.25.
    if NUMPY_VERSION < (1, 25):
        # Older numpy doesn't seem to support `data` in the form of [Point3D(1, 2), Point3D(3, 4)]
        # this happens for python 3.8 (1.25 supports 3.9+)
        from .. import Vec4D

        if isinstance(data, Sequence):
            data = [np.array(p.xyzw) if isinstance(p, Vec4D) else p for p in data]  # type: ignore[assignment]

    points = np.asarray(data, dtype=np.float32).reshape((-1,))
    return pa.FixedSizeListArray.from_arrays(points, type=data_type)
