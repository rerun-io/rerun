from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import numpy as np
import numpy.typing as npt
import pyarrow as pa

if TYPE_CHECKING:
    from . import Mat3x3ArrayLike, Mat3x3Like


class Mat3x3Ext:
    @staticmethod
    def flat_columns__field_converter_override(data: Mat3x3Like) -> npt.NDArray[np.float32]:
        from . import Mat3x3

        if isinstance(data, Mat3x3):
            return data.flat_columns
        else:
            arr = np.array(data, dtype=np.float32).reshape(3, 3)
            return arr.flatten("F")

    @staticmethod
    def native_to_pa_array_override(data: Mat3x3ArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Mat3x3

        # Normalize into list of Mat3x3
        if isinstance(data, Sequence):
            # single matrix made up of flat float array.
            if isinstance(data[0], float | int):
                matrices = [Mat3x3(data)]
            # if there's a sequence nested, either it's several matrices in various formats
            # where the first happens to be either a flat or nested sequence of floats,
            # or it's a single matrix with a nested sequence of floats.
            # for that to be true, the nested sequence must be 3 floats.
            elif (
                isinstance(data[0], Sequence)
                and len(data[0]) == 3
                and all(isinstance(elem, float | int) for elem in data[0])
            ):
                matrices = [Mat3x3(data)]
            # several matrices otherwise!
            else:
                matrices = [Mat3x3(m) for m in data]
        else:
            matrices = [Mat3x3(data)]

        float_arrays = np.asarray([matrix.flat_columns for matrix in matrices], dtype=np.float32).reshape(-1)
        return pa.FixedSizeListArray.from_arrays(float_arrays, type=data_type)
