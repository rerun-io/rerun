from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import pyarrow as pa

if TYPE_CHECKING:
    from . import EntityPathArrayLike


class EntityPathExt:
    """Extension for [EntityPath][rerun.datatypes.EntityPath]."""

    @staticmethod
    def native_to_pa_array_override(data: EntityPathArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, str):
            array = [data]
        elif isinstance(data, Sequence):
            array = [str(datum) for datum in data]
        else:
            array = [str(data)]

        return pa.array(array, type=data_type)
