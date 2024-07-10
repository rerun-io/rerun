from __future__ import annotations

from typing import TYPE_CHECKING, cast

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import ViewCoordinatesArrayLike


class ViewCoordinatesExt:
    """Extension for [ViewCoordinates][rerun.datatypes.ViewCoordinates]."""

    @staticmethod
    def native_to_pa_array_override(data: ViewCoordinatesArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import ViewCoordinates, ViewCoordinatesLike

        if isinstance(data, ViewCoordinates):
            # ViewCoordinates
            data = [data.coordinates]
        elif hasattr(data, "__len__") and len(data) > 0 and isinstance(data[0], ViewCoordinates):  # type: ignore[arg-type, index]
            # [ViewCoordinates]
            data = [d.coordinates for d in data]  # type: ignore[union-attr]
        else:
            data = cast(ViewCoordinatesLike, data)
            try:
                # [x, y, z]
                data = [ViewCoordinates(data).coordinates]
            except ValueError:
                # [[x, y, z], ...]
                data = [ViewCoordinates(d).coordinates for d in data]  # type: ignore[union-attr]

        data = np.asarray(data, dtype=np.uint8)

        if len(data.shape) != 2 or data.shape[1] != 3:
            raise ValueError(f"ViewCoordinates must be a 3-element array. Got: {data.shape}")

        data = data.flatten()

        for value in data:
            # TODO(jleibs): Enforce this validation based on ViewDir
            if value not in range(1, 7):
                raise ValueError("ViewCoordinates must contain only values in the range [1,6].")

        return pa.FixedSizeListArray.from_arrays(data, type=data_type)
