from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

from .._validators import flat_np_float32_array_from_array_like

if TYPE_CHECKING:
    from . import Vec2DArrayLike


NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class Vec2DExt:
    """Extension for [Vec2D][rerun.datatypes.Vec2D]."""

    @staticmethod
    def native_to_pa_array_override(data: Vec2DArrayLike, data_type: pa.DataType) -> pa.Array:
        points = flat_np_float32_array_from_array_like(data, 2)
        return pa.FixedSizeListArray.from_arrays(points, type=data_type)
