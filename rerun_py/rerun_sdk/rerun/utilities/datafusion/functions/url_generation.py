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
    segment_id: str | Expr | None = None,
    timestamp: str | Expr | None = None,
    timeline_name: str | None = None,
    time_range_start: str | Expr | None = None,
    time_range_end: str | Expr | None = None,
    selection: str | Expr | None = None,
) -> Expr:
    """
    Compute the URL for a segment within a dataset.

    This is a Rerun focused DataFusion function that will create a DataFusion
    expression for the segment URL.

    Parameters
    ----------
    dataset:
        The input Rerun Dataset.
    segment_id:
        Expression or column name for the segment ID. If not provided, the column named `rerun_segment_id` will be used.
    timestamp:
        Expression or column name for a timestamp. Generate a URL that specifies the position of the time cursor when
        opened by the viewer.
    timeline_name:
        Specifies which timeline to use when used in combination with `timestamp` and/or `time_range_start`/
        `time_range_end`. By default, this will use the same string as `timestamp` if provided.
    time_range_start:
        Expression or column name for the start of a time range selection. Must be used together with `time_range_end`.
        Generates a URL that specifies a time range to be selected when opened by the viewer.
    time_range_end:
        Expression or column name for the end of a time range selection. Must be used together with `time_range_start`.
        Generates a URL that specifies a time range to be selected when opened by the viewer.
    selection:
        Expression or column name for the data path to select. The syntax is an entity path, optionally
        followed by an instance index and/or component name (e.g. `/world/points`,
        `/world/points[#42]`, `/world/points:Color`, `/world/points[#42]:Color`).
        Generates a URL that specifies the data to be selected when opened by the viewer.

    """
    if not HAS_DATAFUSION:
        raise RerunMissingDependencyError("datafusion", "datafusion")

    if (time_range_start is None) != (time_range_end is None):
        raise ValueError("time_range_start and time_range_end must both be provided or both be omitted")

    if segment_id is None:
        segment_id = col("rerun_segment_id")
    if isinstance(segment_id, str):
        segment_id = col(segment_id)

    rust_udf = _make_rust_udf()

    origin_expr = lit(dataset.catalog.url)
    entry_id_expr = lit(pa.scalar(dataset.id.as_bytes(), type=pa.binary(16)))

    # Derive timeline_name default from timestamp if not explicitly provided
    if timeline_name is None and timestamp is not None:
        timeline_name = str(timestamp)

    # Validate that timeline_name is available when needed
    has_time_range = time_range_start is not None
    if timeline_name is None and (timestamp is not None or has_time_range):
        raise ValueError("timeline_name must be provided when using time_range without timestamp")

    # Build timestamp expression
    if timestamp is not None:
        if isinstance(timestamp, str):
            timestamp = col(timestamp)
        ts_expr = timestamp
    else:
        ts_expr = lit(None)

    # Build timeline expression
    timeline_expr = lit(timeline_name) if timeline_name is not None else lit(None)

    # Build time range expressions
    if time_range_start is not None and time_range_end is not None:
        if isinstance(time_range_start, str):
            time_range_start = col(time_range_start)
        if isinstance(time_range_end, str):
            time_range_end = col(time_range_end)
        range_start_expr = time_range_start
        range_end_expr = time_range_end
    else:
        range_start_expr = lit(None)
        range_end_expr = lit(None)

    # Build selection expression
    if selection is not None:
        if isinstance(selection, str):
            selection = col(selection)
        selection_expr = selection
    else:
        selection_expr = lit(None)

    return rust_udf(
        origin_expr,
        entry_id_expr,
        segment_id,
        ts_expr,
        timeline_expr,
        range_start_expr,
        range_end_expr,
        selection_expr,
    ).alias("segment_url")


def _make_rust_udf() -> ScalarUDF:
    if not HAS_DATAFUSION:
        raise RerunMissingDependencyError("datafusion", "datafusion")

    from rerun_bindings import SegmentUrlUdfInternal  # type: ignore[attr-defined]

    return ScalarUDF.from_pycapsule(SegmentUrlUdfInternal())
