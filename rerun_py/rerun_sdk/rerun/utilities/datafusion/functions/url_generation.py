from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pyarrow.compute

from rerun.error_utils import RerunOptionalDependencyError

if TYPE_CHECKING:
    from rerun_bindings import DatasetEntry

HAS_DATAFUSION = True
try:
    from datafusion import Expr, ScalarUDF, col, udf
except ModuleNotFoundError:
    HAS_DATAFUSION = False


def partition_url(
    dataset: DatasetEntry,
    partition_id_col: str | Expr | None = None,
    timestamp_col: str | Expr | None = None,
    timeline_name: str | None = None,
) -> Expr:
    """
    Compute the URL for a partition within a dataset.

    This is a Rerun focused DataFusion function that will create a DataFusion
    expression for the partition URL.

    To manually invoke the underlying UDF, see `partition_url_udf` or
    `partition_url_with_timeref_udf`.

    Parameters
    ----------
    dataset:
        The input Rerun Dataset.
    partition_id_col:
        The column containing the partition ID. If not provided, it will assume
        a default value of `rerun_partition_id`. You may pass either a DataFusion
        expression or a string column name.
    timestamp_col:
        If this parameter is passed in, generate a URL that will jump to a
        specific timestamp within the partition.
    timeline_name:
        When used in combination with `timestamp_col`, this specifies which timeline
        to seek along. By default this will use the same string as timestamp_col.

    """
    if not HAS_DATAFUSION:
        raise RerunOptionalDependencyError("datafusion", "datafusion")
    if partition_id_col is None:
        partition_id_col = col("rerun_partition_id")
    if isinstance(partition_id_col, str):
        partition_id_col = col(partition_id_col)

    if timestamp_col is not None:
        if timeline_name is None:
            timeline_name = str(timestamp_col)

        if isinstance(timestamp_col, str):
            timestamp_col = col(timestamp_col)

        inner_udf = partition_url_with_timeref_udf(dataset, timeline_name)
        return inner_udf(partition_id_col, timestamp_col).alias("partition_url_with_timestamp")

    inner_udf = partition_url_udf(dataset)
    return inner_udf(partition_id_col).alias("partition_url")


def partition_url_udf(dataset: DatasetEntry) -> ScalarUDF:
    """
    Create a UDF to the URL for a partition within a Dataset.

    This function will generate a UDF that expects one column of input,
    a string containing the Partition ID.
    """
    if not HAS_DATAFUSION:
        raise RerunOptionalDependencyError("datafusion", "datafusion")

    def inner_udf(partition_id_arr: pa.Array) -> pa.Array:
        return pa.compute.binary_join_element_wise(
            dataset.partition_url(""),
            partition_id_arr,
            "",  # Required for join
        )

    return udf(inner_udf, [pa.string()], pa.string(), "stable")


def partition_url_with_timeref_udf(dataset: DatasetEntry, timeline_name: str) -> ScalarUDF:
    """
    Create a UDF to the URL for a partition within a Dataset with timestamp.

    This function will generate a UDF that expects two columns of input,
    a string containing the Partition ID and the timestamp in nanoseconds.
    """
    if not HAS_DATAFUSION:
        raise RerunOptionalDependencyError("datafusion", "datafusion")

    def inner_udf(partition_id_arr: pa.Array, timestamp_arr: pa.Array) -> pa.Array:
        timestamp_us = pa.compute.cast(timestamp_arr, pa.timestamp("us"))

        timestamp_us = pa.compute.strftime(
            timestamp_us,
            "%Y-%m-%dT%H:%M:%SZ",
        )

        return pa.compute.binary_join_element_wise(
            dataset.partition_url(""),
            partition_id_arr,
            f"#when={timeline_name}@",
            timestamp_us,
            "",  # Required for join
        )

    return udf(inner_udf, [pa.string(), pa.timestamp("ns")], pa.string(), "stable")
