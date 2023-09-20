from __future__ import annotations

from enum import IntEnum
from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import ViewCoordinatesArrayLike


class ViewCoordinatesExt:
    class ViewDir(IntEnum):
        UP = 1
        DOWN = 2
        RIGHT = 3
        LEFT = 4
        FORWARD = 5
        BACK = 6

    UP = ViewDir.UP
    DOWN = ViewDir.DOWN
    RIGHT = ViewDir.RIGHT
    LEFT = ViewDir.LEFT
    FORWARD = ViewDir.FORWARD
    BACK = ViewDir.BACK

    @staticmethod
    def native_to_pa_array_override(data: ViewCoordinatesArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import ViewCoordinates

        if isinstance(data, ViewCoordinates):
            data = data.coordinates

        data = np.asarray(data, dtype=np.uint8)

        if data.shape != (3,):
            raise ValueError(f"ViewCoordinates must be a 3-element array. Got: {data.shape}")

        for value in data:
            # TODO(jleibs): Enforce this validation based on ViewDir
            if value not in range(1, 7):
                raise ValueError("ViewCoordinates must contain only values in the range [1,6].")

        return pa.FixedSizeListArray.from_arrays(data, type=data_type)
