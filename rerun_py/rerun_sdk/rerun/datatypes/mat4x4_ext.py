from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, cast

import numpy as np
import pyarrow as pa

from rerun.error_utils import _send_warning

if TYPE_CHECKING:
    from . import Mat4x4ArrayLike, Mat4x4Like


class Mat4x4Ext:
    def __init__(self: Any, rows: Mat4x4Like | None = None, *, columns: Mat4x4Like | None = None) -> None:
        from . import Mat4x4

        if rows is not None:
            if columns is not None:
                _send_warning("Can't specify both columns and rows of matrix.", 1, recording=None)

            if isinstance(rows, Mat4x4):
                self.flat_columns = rows.flat_columns
            else:
                arr = np.array(rows, dtype=np.float32).reshape(4, 4)
                self.flat_columns = arr.flatten("F")
        elif columns is not None:
            # Equalize the format of the columns to a 3x3 matrix.
            # Numpy expects rows _and_ stores row-major. Therefore the flattened list will have flat columns.
            arr = np.array(columns, dtype=np.float32).reshape(4, 4)
            self.flat_columns = arr.flatten()
        else:
            _send_warning("Need to specify either columns or columns of matrix.", 1, recording=None)
            self.flat_columns = np.identity(4, dtype=np.float32).flatten()

    @staticmethod
    def native_to_pa_array_override(data: Mat4x4ArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Mat4x4, Mat4x4Like

        # Normalize into list of Mat4x4
        if isinstance(data, Sequence):
            # single matrix made up of flat float array.
            if isinstance(data[0], float | int):
                matrices = [Mat4x4(cast(Mat4x4Like, data))]
            # if there's a sequence nested, either it's several matrices in various formats
            # where the first happens to be either a flat or nested sequence of floats,
            # or it's a single matrix with a nested sequence of floats.
            # for that to be true, the nested sequence must be 4 floats.
            elif (
                isinstance(data[0], Sequence)
                and len(data[0]) == 4
                and all(isinstance(elem, float | int) for elem in data[0])
            ):
                matrices = [Mat4x4(cast(Mat4x4Like, data))]
            # several matrices otherwise!
            else:
                matrices = [Mat4x4(m) for m in data]
        else:
            matrices = [Mat4x4(data)]

        float_arrays = np.asarray([matrix.flat_columns for matrix in matrices], dtype=np.float32).reshape(-1)
        return pa.FixedSizeListArray.from_arrays(float_arrays, type=data_type)
