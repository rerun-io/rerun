from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    import numpy.typing as npt

    from .. import Point2D, Point2DArrayLike


def point2d_as_array(data: Point2D, dtype: npt.DTypeLike = None) -> npt.ArrayLike:
    return np.array([data.x, data.y], dtype=dtype)


def point2d_native_to_pa_array(data: Point2DArrayLike, data_type: pa.DataType) -> pa.Array:
    points = np.asarray(data, dtype=np.float32).reshape((-1, 2))
    return pa.StructArray.from_arrays(
        arrays=[pa.array(c, type=pa.float32()) for c in points.T],
        fields=list(data_type),
    )
