from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import (
        Utf8Pair,
        Utf8PairArrayLike,
        Utf8PairLike,
    )


def _utf8_pair_converter(data: Utf8PairLike) -> Utf8Pair:
    from . import Utf8Pair

    if isinstance(data, Utf8Pair):
        return data
    # Assume it's a tuple-like (key, value) or dict entry
    elif hasattr(data, "__len__") and len(data) == 2:
        return Utf8Pair(first=data[0], second=data[1])
    else:
        raise ValueError(f"Cannot convert {type(data)} to Utf8Pair")


class Utf8PairExt:
    """Extension for [Utf8Pair][rerun.datatypes.Utf8Pair]."""

    @staticmethod
    def native_to_pa_array_override(data: Utf8PairArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Utf8Batch, Utf8Pair

        if isinstance(data, Utf8Pair):
            first_string_batch = Utf8Batch(data.first)
            second_string_batch = Utf8Batch(data.second)
        elif isinstance(data, np.ndarray):
            # We expect a 2-column array of string-compatible objects
            if len(data.shape) != 2 or data.shape[1] != 2:
                raise ValueError(f"Expected a 2-column numpy array, got an array with shape {data.shape}")
            first_string_batch = Utf8Batch(data[:, 0])
            second_string_batch = Utf8Batch(data[:, 1])
        else:
            # non-numpy Sequence[Utf8Pair | Tuple(Utf8Like, Utf8Like)]
            converted_pairs = [_utf8_pair_converter(item) for item in data]
            first_strings = [pair.first for pair in converted_pairs]
            second_strings = [pair.second for pair in converted_pairs]
            first_string_batch = Utf8Batch(first_strings)
            second_string_batch = Utf8Batch(second_strings)

        return pa.StructArray.from_arrays(
            arrays=[first_string_batch.as_arrow_array(), second_string_batch.as_arrow_array()],
            fields=[data_type.field("first"), data_type.field("second")],
        )
