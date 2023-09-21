from __future__ import annotations

from enum import IntEnum
from typing import TYPE_CHECKING, Iterable

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from ..log import ComponentBatchLike
    from . import ViewCoordinatesArrayLike, ViewCoordinatesLike


class ViewCoordinatesExt:
    class ViewDir(IntEnum):
        Up = 1
        Down = 2
        Right = 3
        Left = 4
        Forward = 5
        Back = 6

    @staticmethod
    def coordinates__field_converter_override(data: ViewCoordinatesLike) -> pa.Array:
        coordinates = np.asarray(data, dtype=np.uint8)
        if coordinates.shape != (3,):
            raise ValueError(f"ViewCoordinates must be a 3-element array. Got: {coordinates.shape}")
        return coordinates

    @staticmethod
    def native_to_pa_array_override(data: ViewCoordinatesArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import ViewCoordinates

        if isinstance(data, ViewCoordinates):
            # ViewCoordinates
            data = [data.coordinates]
        elif hasattr(data, "__len__") and len(data) > 0 and isinstance(data[0], ViewCoordinates):  # type: ignore[arg-type, index]
            # [ViewCoordinates]
            data = [d.coordinates for d in data]  # type: ignore[union-attr]
        else:
            try:
                # [x, y, z]
                data = [ViewCoordinates(data).coordinates]  # type: ignore[arg-type]
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

    # Implement the ArchetypeLike protocol
    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        from ..archetypes import ViewCoordinates

        return ViewCoordinates(self).as_component_batches()

    def num_instances(self) -> int:
        # Always a mono-component
        return 1
