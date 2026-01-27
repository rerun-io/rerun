"""Internal Arrow utilities."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import datafusion
    import pyarrow as pa


def to_record_batch(table: pa.RecordBatch | list[pa.RecordBatch] | datafusion.DataFrame) -> pa.RecordBatch:
    """Convert various table types to a single RecordBatch."""

    import pyarrow as pa

    if isinstance(table, pa.RecordBatch):
        return table
    elif isinstance(table, list):
        return pa.concat_batches(table)
    else:
        # datafusion.DataFrame
        return pa.concat_batches(table.collect())
