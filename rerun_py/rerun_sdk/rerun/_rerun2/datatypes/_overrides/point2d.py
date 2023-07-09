from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    import numpy.typing as npt

    from .. import Point2D, Point2DArrayLike


NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


def point2d_as_array(data: Point2D, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
    return np.array([data.x, data.y], dtype=dtype)


def point2d_native_to_pa_array(data: Point2DArrayLike, data_type: pa.DataType) -> pa.Array:
    from .. import Point2D

    # TODO(ab): get rid of this once we drop support for Python 3.8. Make sure to pin numpy>=1.25.
    if NUMPY_VERSION < (1, 25):
        # Older numpy doesn't seem to support `data` in the form of [Point2D(1, 2), Point2D(3, 4)]
        # this happens for python 3.8 (1.25 supports 3.9+)
        if isinstance(data, Sequence):
            data = [point2d_as_array(p) if isinstance(p, Point2D) else p for p in data]  # type: ignore[assignment]

    points = np.asarray(data, dtype=np.float32).reshape((-1, 2))
    return pa.StructArray.from_arrays(
        arrays=[pa.array(c, type=pa.float32()) for c in points.T],
        fields=list(data_type),
    )
