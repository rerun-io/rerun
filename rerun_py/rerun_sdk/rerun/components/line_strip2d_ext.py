from __future__ import annotations

from collections.abc import Sized
from typing import TYPE_CHECKING, Sequence

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import LineStrip2DArrayLike


def next_offset(acc: int, arr: Sized) -> int:
    return acc + len(arr)


class LineStrip2DExt:
    """Extension for [LineStrip2D][rerun.components.LineStrip2D]."""

    @staticmethod
    def native_to_pa_array_override(data: LineStrip2DArrayLike, data_type: pa.DataType) -> pa.Array:
        from ..datatypes import Vec2DBatch
        from . import LineStrip2D

        # pure-numpy fast path
        if isinstance(data, np.ndarray):
            if len(data) == 0:
                inners = []
            elif data.ndim == 2:
                inners = [Vec2DBatch(data).as_arrow_array().storage]
            else:
                o = 0
                offsets = [o] + [o := next_offset(o, arr) for arr in data]
                inner = Vec2DBatch(data.reshape(-1)).as_arrow_array().storage
                return pa.ListArray.from_arrays(offsets, inner, type=data_type)

        # pure-object
        elif isinstance(data, LineStrip2D):
            inners = [Vec2DBatch(data.points).as_arrow_array().storage]

        # sequences
        elif isinstance(data, Sequence):
            if len(data) == 0:
                inners = []
            elif isinstance(data[0], np.ndarray):
                inners = [Vec2DBatch(datum).as_arrow_array().storage for datum in data]  # type: ignore[arg-type]
            elif isinstance(data[0], LineStrip2D):
                inners = [Vec2DBatch(datum.points).as_arrow_array().storage for datum in data]  # type: ignore[union-attr]
            else:
                inners = [Vec2DBatch(datum).as_arrow_array().storage for datum in data]  # type: ignore[arg-type]

        else:
            inners = [Vec2DBatch(data).storage]

        if len(inners) == 0:
            offsets = pa.array([0], type=pa.int32())
            inner = Vec2DBatch([]).as_arrow_array().storage
            return pa.ListArray.from_arrays(offsets, inner, type=data_type)

        o = 0
        offsets = [o] + [o := next_offset(o, inner) for inner in inners]

        inner = pa.concat_arrays(inners)

        return pa.ListArray.from_arrays(offsets, inner, type=data_type)
