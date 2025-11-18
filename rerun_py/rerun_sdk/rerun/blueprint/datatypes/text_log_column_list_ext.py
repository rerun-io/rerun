from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Any

import pyarrow as pa

if TYPE_CHECKING:
    from collections.abc import Iterable, Sequence

    from ...blueprint import datatypes as blueprint_datatypes
    from .text_log_column_list import TextLogColumnListArrayLike


class TextLogColumnListExt:
    """Extension for [TextLogColumnList][rerun.blueprint.datatypes.TextLogColumnList]."""

    @staticmethod
    def text_log_columns__field_converter_override(
        data: Sequence[blueprint_datatypes.TextLogColumnLike],
    ) -> list[blueprint_datatypes.TextLogColumn]:
        """Convert input data to a list of TextLogColumn objects."""
        from .text_log_column import TextLogColumn

        return [col if isinstance(col, TextLogColumn) else TextLogColumn(col) for col in data]

    @staticmethod
    def native_to_pa_array_override(input_data: TextLogColumnListArrayLike, data_type: pa.DataType) -> pa.Array:
        from .text_log_column import TextLogColumnBatch
        from .text_log_column_list import TextLogColumnList

        if isinstance(input_data, TextLogColumnList):
            data = [input_data]
        else:
            # if we're a sequence, chances are we're the input of a single TextLogColumnList…
            try:
                data = [TextLogColumnList(input_data)]  # type: ignore[arg-type]
            except (ValueError, TypeError):
                # …but it could be that we're a sequence of TextLogColumnList/inputs to TextLogColumnList
                data = [d if isinstance(d, TextLogColumnList) else TextLogColumnList(d) for d in input_data]

        text_log_columns = pa.ListArray.from_arrays(
            offsets=_compute_offsets(d.text_log_columns for d in data),
            values=TextLogColumnBatch(
                list(itertools.chain.from_iterable(d.text_log_columns for d in data)),
            ).as_arrow_array(),
            type=data_type.field(0).type,
        )

        return pa.StructArray.from_arrays(
            [
                text_log_columns,
            ],
            fields=list(data_type),
        )


def _compute_offsets(data: Iterable[Sequence[Any]]) -> list[int]:
    o = 0
    return [o] + [o := o + len(d) for d in data]
