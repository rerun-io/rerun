from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import (
        Utf8Pair,
        Utf8PairArrayLike,
        Utf8PairLike,
    )


def _utf8_pair_converter(
    data: Utf8PairLike,
) -> Utf8Pair:
    from . import Utf8Pair

    if isinstance(data, Utf8Pair):
        return data
    else:
        return Utf8Pair(*data)


class Utf8PairExt:
    """Extension for [Utf8Pair][rerun.datatypes.Utf8Pair]."""

    @staticmethod
    def native_to_pa_array_override(data: Utf8PairArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Utf8Batch, Utf8Pair

        if isinstance(data, Utf8Pair):
            strings = [data]
        else:
            strings = [_utf8_pair_converter(item) for item in data]

        string0 = [pair.first for pair in strings]
        string1 = [pair.second for pair in strings]

        string0_array = Utf8Batch(string0).as_arrow_array()
        string1_array = Utf8Batch(string1).as_arrow_array()

        return pa.StructArray.from_arrays(
            arrays=[string0_array, string1_array],
            fields=[data_type.field("first"), data_type.field("second")],
        )
