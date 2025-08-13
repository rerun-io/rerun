from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Any

import pyarrow as pa

from ... import datatypes
from ...blueprint import components as blueprint_components, datatypes as blueprint_datatypes

if TYPE_CHECKING:
    from collections.abc import Iterable, Sequence

    from .selected_columns import SelectedColumnsArrayLike


class SelectedColumnsExt:
    """Extension for [SelectedColumns][rerun.blueprint.datatypes.SelectedColumns]."""

    def __init__(
        self: Any,
        columns: Sequence[blueprint_datatypes.ComponentColumnSelectorLike | datatypes.Utf8Like],
    ) -> None:
        """
        Create a new instance of the `SelectedColumns` datatype.

        Example:
        ```python
        SelectedColumns(["timeline", "/entity/path:Component"])
        ```

        Parameters
        ----------
        columns:
            The columns to include.

            The column must be either of the timeline, or component kind. Timeline columns can be specified using a
            `str` without any `:`, or an `Utf8`. Component columns can be specified using either a `str` in the form of
            `"/entity/path:Component"`, or a `ComponentColumnSelector`.

        """

        time_columns: list[datatypes.Utf8] = []
        component_columns: list[blueprint_datatypes.ComponentColumnSelector] = []

        for column in columns:
            if isinstance(column, blueprint_components.ComponentColumnSelector):
                component_columns.append(column)
            elif isinstance(column, datatypes.Utf8):
                time_columns.append(column)
            elif isinstance(column, str):
                try:
                    comp_col = blueprint_components.ComponentColumnSelector(spec=column)
                    component_columns.append(comp_col)
                except ValueError:
                    time_columns.append(datatypes.Utf8(column))
            else:
                raise ValueError(f"Unexpected column type: {column}")

        self.__attrs_init__(time_columns=time_columns, component_columns=component_columns)

    @staticmethod
    def native_to_pa_array_override(input_data: SelectedColumnsArrayLike, data_type: pa.DataType) -> pa.Array:
        from ...blueprint.components import ComponentColumnSelectorBatch
        from ...datatypes import Utf8Batch
        from .selected_columns import SelectedColumns

        if isinstance(input_data, SelectedColumns):
            data = [input_data]
        else:
            # if we're a sequence, chances are we the input of a single SelectedColumns…
            try:
                data = [SelectedColumns(input_data)]  # type: ignore[arg-type]
            except ValueError:
                # …but it could be that we're a sequence of SelectedColumns/inputs to SelectedColumns
                data = [d if isinstance(d, SelectedColumns) else SelectedColumns(d) for d in input_data]

        time_columns = pa.ListArray.from_arrays(
            offsets=_compute_offsets(d.time_columns for d in data),
            values=Utf8Batch(
                list(itertools.chain.from_iterable(d.time_columns for d in data)),
            ).as_arrow_array(),
            type=data_type.field(0).type,
        )

        component_columns = pa.ListArray.from_arrays(
            offsets=_compute_offsets(d.component_columns for d in data),
            values=ComponentColumnSelectorBatch(
                list(itertools.chain.from_iterable(d.component_columns for d in data)),
            ).as_arrow_array(),
            type=data_type.field(1).type,
        )

        return pa.StructArray.from_arrays(
            [
                time_columns,
                component_columns,
            ],
            fields=list(data_type),
        )


def _compute_offsets(data: Iterable[Sequence[Any]]) -> list[int]:
    o = 0
    return [o] + [o := o + len(d) for d in data]
