from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np

from rerun.error_utils import _send_warning_or_raise

if TYPE_CHECKING:
    import pyarrow as pa

    from . import Mat3x3ArrayLike, Mat3x3Like


class Mat3x3Ext:
    """Extension for [Mat3x3][rerun.datatypes.Mat3x3]."""

    def __init__(self: Any, rows: Mat3x3Like | None = None, *, columns: Mat3x3Like | None = None) -> None:
        from . import Mat3x3

        if rows is not None:
            if columns is not None:
                _send_warning_or_raise("Can't specify both columns and rows of matrix.", 1, recording=None)

            if isinstance(rows, Mat3x3):
                flat_columns = rows.flat_columns
            else:
                arr = np.asarray(rows, dtype=np.float32).reshape(3, 3)
                flat_columns = arr.ravel("F")
        elif columns is not None:
            # Equalize the format of the columns to a 3x3 matrix.
            # Numpy expects rows _and_ stores row-major. Therefore the flattened list will have flat columns.
            arr = np.asarray(columns, dtype=np.float32).reshape(3, 3)
            flat_columns = arr.ravel("C")
        else:
            _send_warning_or_raise("Need to specify either columns or columns of matrix.", 1, recording=None)
            flat_columns = np.identity(3, dtype=np.float32).ravel()
        self.__attrs_init__(
            flat_columns=flat_columns,
        )

    @staticmethod
    def native_to_pa_array_override(data: Mat3x3ArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Mat3x3

        if isinstance(data, Mat3x3):
            # Single Mat3x3 instance — already stored as flat column-major
            return np.ascontiguousarray(data.flat_columns, dtype=np.float32)

        try:
            arr = np.asarray(data, dtype=np.float32)
        except (ValueError, TypeError):
            # Heterogeneous list (e.g. mixed Mat3x3, lists, numpy arrays)
            # Fall back to per-element conversion
            result = [Mat3x3(d).flat_columns for d in data]  # type: ignore[union-attr, call-overload]
            return np.ascontiguousarray(np.concatenate(result))

        if arr.size == 0:
            return np.empty((0,), dtype=np.float32)

        # Fast path for single matrix (most common case: np.eye(3), flat list of 9)
        if arr.ndim == 1 and arr.size == 9:
            return np.ascontiguousarray(arr.reshape(3, 3).ravel("F"))

        if arr.ndim == 2 and arr.shape == (3, 3):
            return np.ascontiguousarray(arr.ravel("F"))

        # Batch: reshape to (-1, 3, 3), transpose each, flatten
        # This will raise ValueError with a clear message if shape is incompatible
        arr = arr.reshape(-1, 3, 3)
        return np.ascontiguousarray(arr.transpose(0, 2, 1).ravel())
