from __future__ import annotations

from collections.abc import Sequence, Sized
from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import BlobArrayLike


def next_offset(acc: int, arr: Sized) -> int:
    return acc + len(arr)


class BlobExt:
    """Extension for [Blob][rerun.datatypes.Blob]."""

    @staticmethod
    def native_to_pa_array_override(data: BlobArrayLike, data_type: pa.DataType) -> pa.Array:
        from ..datatypes import Blob, BlobBatch

        # someone or something is building things manually, let them!
        if isinstance(data, BlobBatch):
            return data.as_arrow_array()

        # numpy fast path:
        elif isinstance(data, np.ndarray):
            if len(data) == 0:
                return pa.array([], type=pa.binary())
            elif data.ndim == 1:
                return pa.array([np.array(data, dtype=np.uint8).tobytes()], type=pa.binary())
            else:
                return pa.array([np.array(arr, dtype=np.uint8).tobytes() for arr in data], type=pa.binary())

        elif isinstance(data, Blob):
            return pa.array([np.array(data.data, dtype=np.uint8).tobytes()], type=pa.binary())

        elif isinstance(data, bytes):
            return pa.array([data], type=pa.binary())

        elif hasattr(data, "read"):
            return pa.array([data.read()], type=pa.binary())

        elif isinstance(data, Sequence):
            if len(data) == 0:
                return pa.array([], type=pa.binary())
            elif isinstance(data[0], Blob):
                return pa.array([np.array(datum.data, dtype=np.uint8).tobytes() for datum in data], type=pa.binary())  # type: ignore[union-attr]
            elif isinstance(data[0], bytes):
                return pa.array(list(data), type=pa.binary())  # type: ignore[arg-type]
            else:
                return pa.array([np.array(datum, dtype=np.uint8).tobytes() for datum in data], type=pa.binary())

        else:
            return pa.array([np.array(data.data, dtype=np.uint8).tobytes()], type=pa.binary())
