from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from .. import Vec2DArrayLike


def vec2d_native_to_pa_array(data: Vec2DArrayLike, data_type: pa.DataType) -> pa.Array:
    points = np.asarray(data, dtype=np.float32).reshape((-1,))
    return pa.FixedSizeListArray.from_arrays(points, type=data_type)
