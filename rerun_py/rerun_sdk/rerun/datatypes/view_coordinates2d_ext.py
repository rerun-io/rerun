from __future__ import annotations

from typing import TYPE_CHECKING, cast

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import ViewCoordinates2DArrayLike


class ViewCoordinates2DExt:
    """Extension for [ViewCoordinates2D][rerun.datatypes.ViewCoordinates2D]."""

    @staticmethod
    def native_to_pa_array_override(data: ViewCoordinates2DArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import ViewCoordinates2D, ViewCoordinates2DLike

        if isinstance(data, ViewCoordinates2D):
            data = [data.coordinates]
        elif hasattr(data, "__len__") and len(data) > 0 and isinstance(data[0], ViewCoordinates2D):  # type: ignore[arg-type, index]
            data = [d.coordinates for d in data]  # type: ignore[union-attr]
        else:
            data = cast("ViewCoordinates2DLike", data)
            try:
                data = [ViewCoordinates2D(data).coordinates]
            except ValueError:
                data = [ViewCoordinates2D(d).coordinates for d in data]  # type: ignore[union-attr]

        data = np.asarray(data, dtype=np.uint8)

        if len(data.shape) != 2 or data.shape[1] != 2:
            raise ValueError(f"ViewCoordinates2D must be a 2-element array. Got: {data.shape}")

        data = data.flatten()

        for value in data:
            if value not in range(1, 5):
                raise ValueError(
                    "ViewCoordinates2D must contain only values in the range [1,4] (Up, Down, Right, Left)."
                )

        return pa.FixedSizeListArray.from_arrays(data, type=data_type)
