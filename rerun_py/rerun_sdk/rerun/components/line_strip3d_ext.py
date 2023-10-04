from __future__ import annotations

import numbers
from collections.abc import Sized
from typing import TYPE_CHECKING, Any, Sequence

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import LineStrip3DArrayLike


def next_offset(acc: int, arr: Sized) -> int:
    return acc + len(arr)


class LineStrip3DExt:
    """Extension for [LineStrip3D][rerun.components.LineStrip3D]."""

    @staticmethod
    def native_to_pa_array_override(data: LineStrip3DArrayLike, data_type: pa.DataType) -> pa.Array:
        from ..datatypes import Vec3DBatch
        from . import LineStrip3D

        # pure-numpy fast path
        if isinstance(data, np.ndarray):
            if len(data) == 0:
                inners = []
            elif data.ndim == 2:
                inners = [Vec3DBatch(data).as_arrow_array().storage]
            else:
                o = 0
                offsets = [o] + [o := next_offset(o, arr) for arr in data]
                inner = Vec3DBatch(data.reshape(-1)).as_arrow_array().storage
                return pa.ListArray.from_arrays(offsets, inner, type=data_type)

        # pure-object
        elif isinstance(data, LineStrip3D):
            inners = [Vec3DBatch(data.points).as_arrow_array().storage]

        # sequences
        elif isinstance(data, Sequence):
            if len(data) == 0:
                inners = []
            else:
                # Is it a single strip or several?
                # It could be a sequence of the style `[[0, 0, 0], [1, 1, 1]]` which is a single strip.
                if isinstance(data[0], Sequence) and len(data[0]) > 0 and isinstance(data[0][0], numbers.Number):
                    if len(data[0]) == 3:  # type: ignore[arg-type]
                        # If any of the following elements are not sequence of length 2, Vec2DBatch should raise an error.
                        inners = [Vec3DBatch(data).as_arrow_array().storage]  # type: ignore[arg-type]
                    else:
                        raise ValueError(
                            "Expected a sequence of sequences of 3D vectors, but the inner sequence length was not equal to 2."
                        )
                # It could be a sequence of the style `[np.array([0, 0, 0]), np.array([1, 1, 1])]` which is a single strip.
                elif isinstance(data[0], np.ndarray) and data[0].shape == (3,):
                    # If any of the following elements are not sequence of length 3, Vec3DBatch should raise an error.
                    inners = [Vec3DBatch(data).as_arrow_array().storage]  # type: ignore[arg-type]
                # .. otherwise assume that it's several strips.
                else:

                    def to_vec3d_batch(strip: Any) -> Vec3DBatch:
                        if isinstance(strip, LineStrip3D):
                            return Vec3DBatch(strip.points)
                        else:
                            if isinstance(strip, np.ndarray) and (strip.ndim != 2 or strip.shape[1] != 3):
                                raise ValueError(
                                    "Expected a sequence of 3D vectors, instead got array with shape {strip.shape}."
                                )
                            return Vec3DBatch(strip)

                    inners = [to_vec3d_batch(strip).as_arrow_array().storage for strip in data]
        else:
            inners = [Vec3DBatch(data).storage]

        if len(inners) == 0:
            offsets = pa.array([0], type=pa.int32())
            inner = Vec3DBatch([]).as_arrow_array().storage
            return pa.ListArray.from_arrays(offsets, inner, type=data_type)

        o = 0
        offsets = [o] + [o := next_offset(o, inner) for inner in inners]

        inner = pa.concat_arrays(inners)

        return pa.ListArray.from_arrays(offsets, inner, type=data_type)
