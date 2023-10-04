from __future__ import annotations

import numbers
from collections.abc import Sized
from typing import TYPE_CHECKING, Any, Sequence

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
            else:
                # Is it a single strip or several?
                # It could be a sequence of the style `[[0, 0], [1, 1]]` which is a single strip.
                if all(
                    isinstance(elem, Sequence) and len(elem) > 0 and isinstance(elem[0], numbers.Number)
                    for elem in data
                ):
                    if all(len(elem) == 2 for elem in data):  # type: ignore[arg-type]
                        inners = [Vec2DBatch(data).as_arrow_array().storage]  # type: ignore[arg-type]
                    else:
                        raise ValueError(
                            "Expected a sequence of sequences of 2D vectors, but the inner sequence length was not equal to 2."
                        )
                # It could be a sequence of the style `[np.array([0, 0]), np.array([1, 1])]` which is a single strip.
                elif all(isinstance(elem, np.ndarray) and elem.shape == (2,) for elem in data):
                    inners = [Vec2DBatch(data).as_arrow_array().storage]  # type: ignore[arg-type]
                # .. otherwise assume that it's several strips.
                else:

                    def to_vec2d_batch(strip: Any) -> Vec2DBatch:
                        if isinstance(strip, LineStrip2D):
                            return Vec2DBatch(strip.points)
                        else:
                            if isinstance(strip, np.ndarray) and (strip.ndim != 2 or strip.shape[1] != 2):
                                raise ValueError(
                                    "Expected a sequence of 2D vectors, instead got array with shape {strip.shape}."
                                )
                            return Vec2DBatch(strip)

                    inners = [to_vec2d_batch(strip).as_arrow_array().storage for strip in data]
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
