from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

from ...blueprint import components as blueprint_components

if TYPE_CHECKING:
    from .filter_by_event import FilterByEvent, FilterByEventArrayLike, FilterByEventLike


class _NotAFilterByEventLike(BaseException):
    pass


class FilterByEventExt:
    """Extension for [FilterByEvent][rerun.blueprint.datatypes.FilterByEvent]."""

    @staticmethod
    def native_to_pa_array_override(input_data: FilterByEventArrayLike, data_type: pa.DataType) -> pa.Array:
        from ...blueprint.datatypes import ComponentColumnSelectorBatch
        from ...datatypes import BoolBatch

        try:
            data = [_to_filter_by_event(input_data)]  # type: ignore[arg-type]
        except _NotAFilterByEventLike:
            try:
                data = [_to_filter_by_event(d) for d in input_data]  # type: ignore[union-attr]
            except _NotAFilterByEventLike:
                raise ValueError(f"Unexpected input value: {input_data}")

        return pa.StructArray.from_arrays(
            [
                BoolBatch([x.active for x in data]).as_arrow_array().storage,  # type: ignore[misc, arg-type]
                ComponentColumnSelectorBatch(
                    [x.column for x in data],
                )
                .as_arrow_array()
                .storage,  # type: ignore[misc, arg-type]
            ],
            fields=list(data_type),
        )


def _to_filter_by_event(input_data: FilterByEventLike) -> FilterByEvent:
    from .filter_by_event import FilterByEvent

    if isinstance(input_data, FilterByEvent):
        return input_data
    elif isinstance(input_data, str):
        return FilterByEvent(active=True, column=blueprint_components.ComponentColumnSelector(spec=input_data))
    elif isinstance(input_data, blueprint_components.ComponentColumnSelector):
        return FilterByEvent(active=True, column=input_data)
    else:
        raise _NotAFilterByEventLike()
