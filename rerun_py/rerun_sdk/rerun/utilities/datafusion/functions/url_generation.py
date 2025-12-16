from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pyarrow.compute
from typing_extensions import deprecated

from rerun.error_utils import RerunMissingDependencyError

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry

HAS_DATAFUSION = True
try:
    from datafusion import Expr, ScalarUDF, col, udf
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

    if timestamp_col is not None:
        if timeline_name is None:
            timeline_name = str(timestamp_col)

        if isinstance(timestamp_col, str):
            timestamp_col = col(timestamp_col)

        inner_udf = segment_url_with_timeref_udf(dataset, timeline_name)
        return inner_udf(segment_id_col, timestamp_col).alias("segment_url_with_timestamp")

    inner_udf = segment_url_udf(dataset)
    return inner_udf(segment_id_col).alias("segment_url")


@deprecated("Use segment_url() instead")
def partition_url(
    dataset: DatasetEntry,
    *,
    partition_id_col: str | Expr | None = None,
    timestamp_col: str | Expr | None = None,
    timeline_name: str | None = None,
) -> Expr:
    """Compute the URL for a partition within a dataset."""
    return segment_url(
        dataset,
        segment_id_col=partition_id_col,
        timestamp_col=timestamp_col,
        timeline_name=timeline_name,
    )


def segment_url_udf(dataset: DatasetEntry) -> ScalarUDF:
    """
    Create a UDF to the URL for a segment within a Dataset.

    This function will generate a UDF that expects one column of input,
    a string containing the segment ID.
    """
    if not HAS_DATAFUSION:
        raise RerunMissingDependencyError("datafusion", "datafusion")

    def inner_udf(segment_id_arr: pa.Array) -> pa.Array:
        return pa.compute.binary_join_element_wise(
            dataset.segment_url(""),
            segment_id_arr,
            "",  # Required for join
        )

    return udf(inner_udf, [pa.string()], pa.string(), "stable")


@deprecated("Use segment_url_udf() instead")
def partition_url_udf(dataset: DatasetEntry) -> ScalarUDF:
    """Create a UDF to the URL for a partition within a Dataset."""
    return segment_url_udf(dataset)


def segment_url_with_timeref_udf(dataset: DatasetEntry, timeline_name: str) -> ScalarUDF:
    """
    Create a UDF to the URL for a segment within a Dataset with timestamp.

    This function will generate a UDF that expects two columns of input,
    a string containing the segment ID and the timestamp in nanoseconds.
    """
    if not HAS_DATAFUSION:
        raise RerunMissingDependencyError("datafusion", "datafusion")

    def inner_udf(segment_id_arr: pa.Array, timestamp_arr: pa.Array) -> pa.Array:
        # The choice of `ceil_temporal` is important since this timestamp drives a cursor
        # selection. Due to Rerun latest-at semantics, in order for data from the provided
        # timestamp to be visible, the cursor must be set to a point in time which is
        # greater than or equal to the target.
        timestamp_us = pa.compute.ceil_temporal(timestamp_arr, unit="microsecond")

        timestamp_us = pa.compute.strftime(
            timestamp_us,
            "%Y-%m-%dT%H:%M:%SZ",
        )

        return pa.compute.binary_join_element_wise(
            dataset.segment_url(""),
            segment_id_arr,
            f"#when={timeline_name}@",
            timestamp_us,
            "",  # Required for join
        )

    return udf(inner_udf, [pa.string(), pa.timestamp("ns")], pa.string(), "stable")


@deprecated("Use segment_url_with_timeref_udf() instead")
def partition_url_with_timeref_udf(dataset: DatasetEntry, timeline_name: str) -> ScalarUDF:
    """Create a UDF to the URL for a partition within a Dataset with timestamp."""
    return segment_url_with_timeref_udf(dataset, timeline_name)
