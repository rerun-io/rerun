from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

from ...blueprint import components as blueprint_components

if TYPE_CHECKING:
    from .filter_is_not_null import FilterIsNotNull, FilterIsNotNullArrayLike, FilterIsNotNullLike


class _NotAFilterByEventLike(BaseException):
    pass


class FilterIsNotNullExt:
    """Extension for [FilterIsNotNull][rerun.blueprint.datatypes.FilterIsNotNull]."""

    @staticmethod
    def native_to_pa_array_override(input_data: FilterIsNotNullArrayLike, data_type: pa.DataType) -> pa.Array:
        from ...blueprint.datatypes import ComponentColumnSelectorBatch
        from ...datatypes import BoolBatch

        try:
            data = [_to_filter_by_event(input_data)]  # type: ignore[arg-type]
        except _NotAFilterByEventLike:
            try:
                data = [_to_filter_by_event(d) for d in input_data]  # type: ignore[union-attr]
            except _NotAFilterByEventLike:
                raise ValueError(f"Unexpected input value: {input_data}") from None

        return pa.StructArray.from_arrays(
            [
                BoolBatch([x.active for x in data]).as_arrow_array(),
                ComponentColumnSelectorBatch(
                    [x.column for x in data],
                ).as_arrow_array(),
            ],
            fields=list(data_type),
        )


def _to_filter_by_event(input_data: FilterIsNotNullLike) -> FilterIsNotNull:
    from .filter_is_not_null import FilterIsNotNull

    if isinstance(input_data, FilterIsNotNull):
        return input_data
    elif isinstance(input_data, str):
        return FilterIsNotNull(active=True, column=blueprint_components.ComponentColumnSelector(spec=input_data))
    elif isinstance(input_data, blueprint_components.ComponentColumnSelector):
        return FilterIsNotNull(active=True, column=input_data)
    else:
        raise _NotAFilterByEventLike()
