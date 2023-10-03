from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import pyarrow as pa

if TYPE_CHECKING:
    from . import Utf8ArrayLike


class Utf8Ext:
    """Extension for [Utf8][rerun.datatypes.Utf8]."""

    @staticmethod
    def native_to_pa_array_override(data: Utf8ArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, str):
            array = [data]
        elif isinstance(data, Sequence):
            array = [str(datum) for datum in data]
        else:
            array = [str(data)]

        return pa.array(array, type=data_type)
