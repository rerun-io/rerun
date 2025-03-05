from __future__ import annotations

import numbers
from collections.abc import Sequence, Sized
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import GeoLineStringArrayLike, GeoLineStringLike


def next_offset(acc: int, arr: Sized) -> int:
    return acc + len(arr)


class GeoLineStringExt:
    """Extension for [GeoLineString][rerun.components.GeoLineString]."""

    # TODO(ab): the only purpose of this override is to make the `lat_lon` arg kw-only. Should be codegen-able?
    def __init__(self: Any, *, lat_lon: GeoLineStringLike) -> None:
        """Create a new instance of the GeoLineString component."""

        # You can define your own __init__ function as a member of GeoLineStringExt in geo_line_string_ext.py
        self.__attrs_init__(lat_lon=lat_lon)

    @staticmethod
    def native_to_pa_array_override(data: GeoLineStringArrayLike, data_type: pa.DataType) -> pa.Array:
        from ..datatypes import DVec2DBatch
        from . import GeoLineString

        # pure-numpy fast path
        if isinstance(data, np.ndarray):
            if len(data) == 0:
                inners = []
            elif data.ndim == 2:
                inners = [DVec2DBatch(data).as_arrow_array()]
            else:
                o = 0
                offsets = [o] + [o := next_offset(o, arr) for arr in data]
                inner = DVec2DBatch(data.reshape(-1)).as_arrow_array()
                return pa.ListArray.from_arrays(offsets, inner, type=data_type)

        # pure-object
        elif isinstance(data, GeoLineString):
            inners = [DVec2DBatch(data.lat_lon).as_arrow_array()]

        # sequences
        elif isinstance(data, Sequence):
            if len(data) == 0:
                inners = []
            else:
                # Is it a single strip or several?
                # It could be a sequence of the style `[[0, 0], [1, 1]]` which is a single strip.
                if isinstance(data[0], Sequence) and len(data[0]) > 0 and isinstance(data[0][0], numbers.Number):
                    if len(data[0]) == 2:
                        # If any of the following elements are not sequence of length 2, DVec2DBatch should raise an error.
                        inners = [DVec2DBatch(data).as_arrow_array()]  # type: ignore[arg-type]
                    else:
                        raise ValueError(
                            "Expected a sequence of sequences of 2D vectors, but the inner sequence length was not equal to 2.",
                        )
                # It could be a sequence of the style `[np.array([0, 0]), np.array([1, 1])]` which is a single strip.
                elif isinstance(data[0], np.ndarray) and data[0].shape == (2,):
                    # If any of the following elements are not np arrays of shape 2, DVec2DBatch should raise an error.
                    inners = [DVec2DBatch(data).as_arrow_array()]  # type: ignore[arg-type]
                # .. otherwise assume that it's several strips.
                else:

                    def to_dvec2D_batch(strip: Any) -> DVec2DBatch:
                        if isinstance(strip, GeoLineString):
                            return DVec2DBatch(strip.lat_lon)
                        else:
                            if isinstance(strip, np.ndarray) and (strip.ndim != 2 or strip.shape[1] != 2):
                                raise ValueError(
                                    f"Expected a sequence of 2D vectors, instead got array with shape {strip.shape}.",
                                )
                            return DVec2DBatch(strip)

                    inners = [to_dvec2D_batch(strip).as_arrow_array() for strip in data]
        else:
            inners = [DVec2DBatch(data)]

        if len(inners) == 0:
            offsets = pa.array([0], type=pa.int32())
            inner = DVec2DBatch([]).as_arrow_array()
            return pa.ListArray.from_arrays(offsets, inner, type=data_type)

        o = 0
        offsets = [o] + [o := next_offset(o, inner) for inner in inners]

        inner = pa.concat_arrays(inners)

        return pa.ListArray.from_arrays(offsets, inner, type=data_type)
