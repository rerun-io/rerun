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

        # pure-numpy fast path
        elif isinstance(data, np.ndarray):
            if len(data) == 0:
                inners = []
            elif data.ndim == 1:
                inners = [pa.array(np.array(data, dtype=np.uint8).flatten())]
            else:
                o = 0
                offsets = [o] + [o := next_offset(o, arr) for arr in data]
                inner = pa.array(np.array(data, dtype=np.uint8).flatten())
                return pa.ListArray.from_arrays(offsets, inner, type=data_type)

        # pure-object
        elif isinstance(data, Blob):
            inners = [pa.array(np.array(data.data, dtype=np.uint8).flatten())]

        # pure-bytes
        elif isinstance(data, bytes):
            inners = [pa.array(np.frombuffer(data, dtype=np.uint8))]

        elif hasattr(data, "read"):
            inners = [pa.array(np.frombuffer(data.read(), dtype=np.uint8))]

        # sequences
        elif isinstance(data, Sequence):
            if len(data) == 0:
                inners = []
            elif isinstance(data[0], Blob):
                inners = [pa.array(np.array(datum.data, dtype=np.uint8).flatten()) for datum in data]  # type: ignore[union-attr]
            elif isinstance(data[0], bytes):
                inners = [pa.array(np.frombuffer(datum, dtype=np.uint8)) for datum in data]  # type: ignore[arg-type]
            else:
                inners = [pa.array(np.array(datum, dtype=np.uint8).flatten()) for datum in data]

        else:
            inners = [pa.array(np.array(data.data, dtype=np.uint8).flatten())]

        if len(inners) == 0:
            offsets = pa.array([0], type=pa.int32())
            inner = np.array([], dtype=np.uint8).flatten()
            return pa.ListArray.from_arrays(offsets, inner, type=data_type)

        o = 0
        offsets = [o] + [o := next_offset(o, inner) for inner in inners]

        inner = pa.concat_arrays(inners)

        return pa.ListArray.from_arrays(offsets, inner, type=data_type)
