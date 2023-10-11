from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa

from rerun.error_utils import _send_warning_or_raise

if TYPE_CHECKING:
    from . import Mat4x4ArrayLike, Mat4x4Like


class Mat4x4Ext:
    """Extension for [Mat4x4][rerun.datatypes.Mat4x4]."""

    def __init__(self: Any, rows: Mat4x4Like | None = None, *, columns: Mat4x4Like | None = None) -> None:
        from . import Mat4x4

        if rows is not None:
            if columns is not None:
                _send_warning_or_raise("Can't specify both columns and rows of matrix.", 1, recording=None)

            if isinstance(rows, Mat4x4):
                self.flat_columns = rows.flat_columns
            else:
                arr = np.array(rows, dtype=np.float32).reshape(4, 4)
                self.flat_columns = arr.flatten("F")
        elif columns is not None:
            # Equalize the format of the columns to a 4x4 matrix.
            # Numpy expects rows _and_ stores row-major. Therefore the flattened list will have flat columns.
            arr = np.array(columns, dtype=np.float32).reshape(4, 4)
            self.flat_columns = arr.flatten()
        else:
            _send_warning_or_raise("Need to specify either columns or columns of matrix.", 1, recording=None)
            self.flat_columns = np.identity(4, dtype=np.float32).flatten()

    @staticmethod
    def native_to_pa_array_override(data: Mat4x4ArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Mat4x4

        if isinstance(data, Mat4x4):
            matrices = [data]
        elif len(data) == 0:
            matrices = []
        else:
            try:
                # Try to convert it to a single Mat4x4
                # Will raise ValueError if the wrong shape
                matrices = [Mat4x4(data)]  # type: ignore[arg-type]
            except ValueError:
                # Otherwise try to convert it to a sequence of Mat4x4
                # Let this value error propagate as the fallback
                matrices = [Mat4x4(d) for d in data]

        float_arrays = np.asarray([matrix.flat_columns for matrix in matrices], dtype=np.float32).reshape(-1)
        return pa.FixedSizeListArray.from_arrays(float_arrays, type=data_type)
