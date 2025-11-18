from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Any

import pyarrow as pa

if TYPE_CHECKING:
    from collections.abc import Iterable, Sequence

    from ... import datatypes
    from .text_log_level_list import TextLogLevelListArrayLike


class TextLogLevelListExt:
    """Extension for [TextLogLevelList][rerun.blueprint.datatypes.TextLogLevelList]."""

    @staticmethod
    def log_levels__field_converter_override(
        data: Sequence[datatypes.Utf8Like],
    ) -> list[datatypes.Utf8]:
        """Convert input data to a list of Utf8 objects."""
        from ...datatypes import Utf8

        return [item if isinstance(item, Utf8) else Utf8(item) for item in data]

    @staticmethod
    def native_to_pa_array_override(input_data: TextLogLevelListArrayLike, data_type: pa.DataType) -> pa.Array:
        from ...datatypes import Utf8Batch
        from .text_log_level_list import TextLogLevelList

        if isinstance(input_data, TextLogLevelList):
            data = [input_data]
        else:
            # if we're a sequence, chances are we're the input of a single TextLogLevelList…
            try:
                data = [TextLogLevelList(input_data)]  # type: ignore[arg-type]
            except (ValueError, TypeError):
                # …but it could be that we're a sequence of TextLogLevelList/inputs to TextLogLevelList
                data = [d if isinstance(d, TextLogLevelList) else TextLogLevelList(d) for d in input_data]

        log_levels = pa.ListArray.from_arrays(
            offsets=_compute_offsets(d.log_levels for d in data),
            values=Utf8Batch(
                list(itertools.chain.from_iterable(d.log_levels for d in data)),
            ).as_arrow_array(),
            type=data_type.field(0).type,
        )

        return pa.StructArray.from_arrays(
            [
                log_levels,
            ],
            fields=list(data_type),
        )


def _compute_offsets(data: Iterable[Sequence[Any]]) -> list[int]:
    o = 0
    return [o] + [o := o + len(d) for d in data]
