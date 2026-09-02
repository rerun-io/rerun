"""Shared helpers for integration tests."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import datafusion
    import pyarrow as pa


def normalized_fields(schema: pa.Schema) -> list[tuple[str, pa.DataType, dict[bytes, bytes]]]:
    """`(name, type, per-field metadata)` triplets sorted by name; ignores table-level metadata."""

    return sorted([(f.name, f.type, dict(f.metadata or {})) for f in schema])


def row_multiset(df: datafusion.DataFrame) -> list[str]:
    """Convert every row to a deterministic Python `repr` and return sorted."""

    tbl = df.to_arrow_table().combine_chunks()
    names = sorted(tbl.column_names)
    cols = [tbl.column(n).to_pylist() for n in names]
    return sorted(repr(row) for row in zip(*cols, strict=True))
