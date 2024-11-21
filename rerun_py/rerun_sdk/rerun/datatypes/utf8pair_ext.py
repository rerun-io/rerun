from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import (
        Utf8PairArrayLike,
    )


class Utf8PairExt:
    """Extension for [Utf8Pair][rerun.datatypes.Utf8Pair]."""

    @staticmethod
    def native_to_pa_array_override(data: Utf8PairArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Utf8, Utf8Batch, Utf8Pair

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
            first_strings: list[Utf8 | str] = []
            second_strings: list[Utf8 | str] = []
            for item in data:
                if isinstance(item, Utf8Pair):
                    first_strings.append(item.first)
                    second_strings.append(item.second)
                else:
                    first_strings.append(item[0])
                    second_strings.append(item[1])
            first_string_batch = Utf8Batch(first_strings)
            second_string_batch = Utf8Batch(second_strings)

        return pa.StructArray.from_arrays(
            arrays=[first_string_batch.as_arrow_array(), second_string_batch.as_arrow_array()],
            fields=[data_type.field("first"), data_type.field("second")],
        )
