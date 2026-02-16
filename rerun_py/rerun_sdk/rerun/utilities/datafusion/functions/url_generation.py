from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

from rerun.error_utils import RerunMissingDependencyError

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry

HAS_DATAFUSION = True
try:
    from datafusion import Expr, ScalarUDF, col, lit
except ModuleNotFoundError:
    HAS_DATAFUSION = False


def segment_url(
    dataset: DatasetEntry,
    *,
    segment_id_col: str | Expr | None = None,
    timestamp_col: str | Expr | None = None,
    timeline_name: str | None = None,
) -> Expr:
    """
    Compute the URL for a segment within a dataset.

    This is a Rerun focused DataFusion function that will create a DataFusion
    expression for the segment URL.

    To manually invoke the underlying UDF, see `segment_url_udf` or
    `segment_url_with_timeref_udf`.

    Parameters
    ----------
    dataset:
        The input Rerun Dataset.
    segment_id_col:
        The column containing the segment ID. If not provided, it will assume
        a default value of `rerun_segment_id`. You may pass either a DataFusion
        expression or a string column name.
    timestamp_col:
        If this parameter is passed in, generate a URL that will jump to a
        specific timestamp within the segment.
    timeline_name:
        When used in combination with `timestamp_col`, this specifies which timeline
        to seek along. By default this will use the same string as timestamp_col.

    """
    if not HAS_DATAFUSION:
        raise RerunMissingDependencyError("datafusion", "datafusion")

    if segment_id_col is None:
        segment_id_col = col("rerun_segment_id")
    if isinstance(segment_id_col, str):
        segment_id_col = col(segment_id_col)

    rust_udf = _make_rust_udf()

    origin_expr = lit(dataset.catalog.url)
    entry_id_expr = lit(pa.scalar(dataset.id.as_bytes(), type=pa.binary(16)))

    if timestamp_col is not None:
        if timeline_name is None:
            timeline_name = str(timestamp_col)

        if isinstance(timestamp_col, str):
            timestamp_col = col(timestamp_col)

        return rust_udf(origin_expr, entry_id_expr, segment_id_col, timestamp_col, lit(timeline_name)).alias(
            "segment_url"
        )

    return rust_udf(origin_expr, entry_id_expr, segment_id_col, lit(None), lit(None)).alias("segment_url")


def _make_rust_udf() -> ScalarUDF:
    if not HAS_DATAFUSION:
        raise RerunMissingDependencyError("datafusion", "datafusion")

    from rerun_bindings import SegmentUrlUdfInternal  # type: ignore[attr-defined]

    return ScalarUDF.from_pycapsule(SegmentUrlUdfInternal())
