from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from .column_order import ColumnOrderArrayLike


class ColumnOrderExt:
    """Extension for [ColumnOrder][rerun.blueprint.components.ColumnOrder]."""

    @staticmethod
    def native_to_pa_array_override(input_data: ColumnOrderArrayLike, data_type: pa.DataType) -> pa.Array:
        from .column_order import ColumnOrder

        if isinstance(input_data, ColumnOrder):
            data = [input_data]
        else:
            # Try treating the whole input as a single ColumnOrder (e.g. a list of entity path strings).
            try:
                data = [ColumnOrder(input_data)]  # type: ignore[arg-type]
            except (ValueError, TypeError):
                # Otherwise treat it as a sequence of ColumnOrder-like items.
                data = [d if isinstance(d, ColumnOrder) else ColumnOrder(d) for d in input_data]

        return pa.array(
            [[str(ep) for ep in d.entity_paths] for d in data],
            type=data_type,
        )
