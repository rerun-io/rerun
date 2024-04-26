from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

from .._validators import flat_np_float64_array_from_array_like

if TYPE_CHECKING:
    from . import Range1DArrayLike


class Range1DExt:
    """Extension for [Range1D][rerun.datatypes.Range1D]."""

    @staticmethod
    def native_to_pa_array_override(data: Range1DArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, slice):
            if data.step is not None and data.step != 1:
                raise ValueError("Steps other than 1 are not supported for Range1D.")
            data = [data.start, data.stop]

        ranges = flat_np_float64_array_from_array_like(data, 2)
        return pa.FixedSizeListArray.from_arrays(ranges, type=data_type)
