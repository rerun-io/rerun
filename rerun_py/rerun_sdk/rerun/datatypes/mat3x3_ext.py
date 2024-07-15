from __future__ import annotations

import numbers
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa

from rerun.error_utils import _send_warning_or_raise

if TYPE_CHECKING:
    from . import Mat3x3ArrayLike, Mat3x3Like


class Mat3x3Ext:
    """Extension for [Mat3x3][rerun.datatypes.Mat3x3]."""

    def __init__(self: Any, rows: Mat3x3Like | None = None, *, columns: Mat3x3Like | None = None) -> None:
        from . import Mat3x3

        if rows is not None:
            if columns is not None:
                _send_warning_or_raise("Can't specify both columns and rows of matrix.", 1, recording=None)

            if isinstance(rows, Mat3x3):
                self.flat_columns = rows.flat_columns
            else:
                arr = np.array(rows, dtype=np.float32).reshape(3, 3)
                self.flat_columns = arr.flatten("F")
        elif columns is not None:
            # Equalize the format of the columns to a 3x3 matrix.
            # Numpy expects rows _and_ stores row-major. Therefore the flattened list will have flat columns.
            arr = np.array(columns, dtype=np.float32).reshape(3, 3)
            self.flat_columns = arr.flatten("C")
        else:
            _send_warning_or_raise("Need to specify either columns or columns of matrix.", 1, recording=None)
            self.flat_columns = np.identity(3, dtype=np.float32).flatten()

    @staticmethod
    def native_to_pa_array_override(data: Mat3x3ArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Mat3x3

        if isinstance(data, Mat3x3):
            matrices = [data]
        elif len(data) == 0:  # type: ignore[arg-type]
            matrices = []
        else:
            try:
                # Try to convert it to a single Mat3x3
                # Will raise ValueError if the wrong shape
                matrices = [Mat3x3(data)]  # type: ignore[arg-type]
            except ValueError:
                # If the data can't be possibly more than one Mat3x3, raise the original ValueError.
                if isinstance(data[0], numbers.Number):  # type: ignore[arg-type, index]
                    raise

                # Otherwise try to convert it to a sequence of Mat3x3s
                # Let this value error propagate as the fallback
                matrices = [Mat3x3(d) for d in data]  # type: ignore[arg-type, union-attr]

        float_arrays = np.asarray([matrix.flat_columns for matrix in matrices], dtype=np.float32).reshape(-1)
        return pa.FixedSizeListArray.from_arrays(float_arrays, type=data_type)
