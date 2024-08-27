from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import pyarrow as pa

if TYPE_CHECKING:
    from . import Utf8ListArrayLike


class Utf8ListExt:
    """Extension for [Utf8List][rerun.blueprint.datatypes.Utf8List]."""

    @staticmethod
    def value__field_converter_override(value: str | list[str]) -> list[str]:
        if isinstance(value, str):
            return [value]
        return value

    @staticmethod
    def native_to_pa_array_override(data: Utf8ListArrayLike, data_type: pa.DataType) -> pa.Array:
        from ..datatypes import Utf8List

        if isinstance(data, Utf8List):
            data = data.value

        if isinstance(data, str):
            array = [[data]]
        elif isinstance(data, Sequence):
            data = list(data)
            if len(data) == 0:
                array = []
            elif isinstance(data[0], Utf8List):
                # List of Utf8List!
                array = [datum.value for datum in data]  # type: ignore[union-attr]
            elif isinstance(data[0], Sequence):
                # It's a nested sequence. Might still be a string though since strings are sequences.
                if isinstance(data[0], str):
                    array = [[str(datum) for datum in data]]
                else:
                    array = [[str(item) for item in sub_array] for sub_array in data]  # type: ignore[union-attr]
            else:
                array = [[str(datum) for datum in data]]
        else:
            array = [[str(data)]]

        return pa.array(array, type=data_type)
