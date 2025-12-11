from __future__ import annotations

import logging
import uuid
from typing import TYPE_CHECKING

import datafusion as dfn
import numpy as np
import pyarrow as pa
import pytest
import rerun as rr
from datafusion import col, functions as F

if TYPE_CHECKING:
    from pytest_benchmark.fixture import BenchmarkFixture
    from rerun.catalog import CatalogClient, DataframeQueryView, DatasetEntry

logger = logging.getLogger(__name__)

pytestmark = [pytest.mark.cloud_only, pytest.mark.slow]


@pytest.mark.benchmark(group="droid")
def test_count_gripper_closes(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Count the number of gripper segments in the dataset."""
    benchmark.pedantic(
        count_gripper_closes,
        args=(dataset,),
        rounds=1,
    )


def count_gripper_closes(dataset: DatasetEntry) -> None:
    """Count the number of gripper segments in the dataset."""

    robot = dataset.dataframe_query_view(index="real_time", contents="/action/**")

    count_filter = dfn.col("gripper_position")[0] > 0.3
    grip_time_table = (
        robot.fill_latest_at()
        .df()
        .select("rerun_segment_id", dfn.col("/action/gripper_position:Scalars:scalars").alias("gripper_position"))
        .aggregate([dfn.col("rerun_segment_id")], [dfn.functions.count(filter=count_filter).alias("grip_time")])
        .sort(dfn.col("grip_time"))
    )

    def segment_url(segment_id_arr: list[str]) -> pa.Array:
        """Create a URL for linking back to the segment from inside the viewer."""
        return pa.array(f"<fake>/dataset/{dataset.id}?segment_id={sid}" for sid in segment_id_arr)

    segment_url_udf = dfn.udf(segment_url, [pa.string()], pa.string(), "stable")

    grip_time_table = grip_time_table.with_column(
        "segment_url",
        segment_url_udf(dfn.col("rerun_segment_id")),
    )

    results = grip_time_table.collect()
    assert len(results) > 0, "expected at least one record batch from results"
    assert results[0].num_rows > 0, "expected at least one row from results"


@pytest.mark.benchmark(group="droid")
def test_aggregate_and_self_join(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Ensure our segment hashing works effectively by self joining."""
    benchmark.pedantic(
        aggregate_and_self_join_body,
        args=(dataset,),
        rounds=1,
    )


def aggregate_and_self_join_body(dataset: DatasetEntry) -> None:
    """Ensure our segment hashing works effectively by self joining."""
    robot = dataset.dataframe_query_view(index="real_time", contents="/action/**")

    gripper_filter = dfn.col("gripper_position")[0] > 0.3
    df = (
        robot.fill_latest_at()
        .df()
        .select("rerun_segment_id", dfn.col("/action/gripper_position:Scalars:scalars").alias("gripper_position"))
    )

    df_first_grip = df.aggregate(
        [dfn.col("rerun_segment_id")],
        [dfn.functions.first_value(dfn.col("gripper_position"), filter=gripper_filter).alias("first_grip")],
    ).with_column_renamed("rerun_segment_id", "left_rerun_segment_id")
    df = df.join(df_first_grip, right_on="rerun_segment_id", left_on="left_rerun_segment_id")

    results = df.collect()
    assert len(results) > 0, "expected at least one record batch"


@pytest.mark.benchmark(group="droid")
def test_segment_time_ordering(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Benchmark to measure performance of the time ordering since a sort should not be needed during aggregation."""
    benchmark.pedantic(
        segment_time_ordering_body,
        args=(dataset,),
        rounds=1,
    )


def segment_time_ordering_body(dataset: DatasetEntry) -> None:
    """Benchmark to measure performance of the time ordering since a sort should not be needed during aggregation."""
    robot = dataset.dataframe_query_view(index="real_time", contents="/action/**")

    df = robot.df().aggregate(
        dfn.col("rerun_segment_id"),
        dfn.functions.first_value(dfn.col("real_time"), order_by=[dfn.col("real_time")]),
    )

    results = df.collect()
    assert len(results) > 0


@pytest.mark.benchmark(group="droid")
def test_create_vector_index(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Create a vector index for the embeddings columns."""
    benchmark.pedantic(
        create_vector_index_body,
        args=(dataset,),
        rounds=1,
    )


def create_vector_index_body(dataset: DatasetEntry) -> None:
    """Create a vector index for the embeddings columns."""
    embedding_column = rr.dataframe.ComponentColumnSelector("/camera/wrist/embedding", "embeddings")

    try:
        dataset.delete_indexes(column=embedding_column)
    except Exception as e:
        # That's OK, the index may legitimately not exist
        logger.info(f"Failed to delete index: {e}")

    dataset.create_vector_index(
        column=embedding_column,
        time_index=rr.dataframe.IndexColumnSelector("real_time"),
        # targeting ~5 segments.
        # droid:raw has 640213 vectors, dividing it by 2**17
        # gives us that
        target_partition_num_rows=131072,
        num_sub_vectors=16,
        distance_metric="Cosine",
    )


@pytest.mark.benchmark(group="droid")
def test_perform_vector_search(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Perform a vector search for a specific embedding."""
    benchmark.pedantic(
        perform_vector_search_body,
        args=(dataset,),
        rounds=1,
    )


def perform_vector_search_body(dataset: DatasetEntry) -> None:
    """Perform a vector search for a specific embedding."""
    # This works with droid:raw since the query is hard coded
    embedding_column = rr.dataframe.ComponentColumnSelector("/camera/wrist/embedding", "embeddings")
    result = dataset.search_vector([0.0] * 768, embedding_column, top_k=10).df().collect()
    assert len(result) > 0


@pytest.mark.benchmark(group="droid")
def test_lookup_embedding_using_index_values(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Look up the embedding for a specific time."""
    benchmark.pedantic(
        lookup_embedding_using_index_values_body,
        args=(dataset,),
        rounds=1,
    )


def lookup_embedding_using_index_values_body(dataset: DatasetEntry) -> None:
    """Look up the embedding for a specific time."""

    # This works with droid:raw
    selected_time = 1692335046618897920

    # Currently `using_index_values` will actually give us a single result, and so we have the option
    # to use that to improve the performance of this query.
    # TODO(https://linear.app/rerun/issue/DPF-1818/): Decide if this is actually something we want to depend on

    result = (
        dataset.dataframe_query_view(index="real_time", contents="/camera/wrist/embedding")
        .using_index_values([selected_time])
        .fill_latest_at()
        .df()
        .select("/camera/wrist/embedding:embeddings")
    ).collect()
    assert len(result) > 0


@pytest.mark.benchmark(group="droid")
def test_sample_index_values(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Count the number of gripper segments in the dataset."""
    benchmark.pedantic(
        sample_index_values_body,
        args=(dataset,),
        rounds=1,
    )


def sample_index_values_body(dataset: DatasetEntry) -> None:
    """Count the number of gripper segments in the dataset."""
    wrist = dataset.dataframe_query_view(
        index="log_tick",
        contents="/camera/wrist/embedding /thumbnail/camera/wrist",
    )

    sampled_times = [0, 100, 200, 500, 1000, 2000]
    result = (wrist.filter_index_values(sampled_times).fill_latest_at()).df().drop("rerun_segment_id").collect()
    assert len(result) > 0


@pytest.mark.benchmark(group="droid")
def test_sample_index_values_chunk_ids(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Find the Chunk IDs needed for `sample_index_values`."""
    benchmark.pedantic(
        sample_index_values_chunk_ids_body,
        args=(dataset,),
        rounds=1,
    )


def sample_index_values_chunk_ids_body(dataset: DatasetEntry) -> None:
    """Find the Chunk IDs needed for `sample_index_values`."""
    wrist = dataset.dataframe_query_view(
        index="log_tick",
        contents="/camera/wrist/embedding /thumbnail/camera/wrist",
    )

    sampled_times = [0, 100, 200, 500, 1000, 2000]
    results = (wrist.filter_index_values(sampled_times).fill_latest_at()).get_chunk_ids()
    for batch in results:
        assert batch.num_rows > 0


@pytest.mark.benchmark(group="droid")
def test_align_fixed_frequency(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Align two columns to a fixed frequency."""
    benchmark.pedantic(
        align_fixed_frequency_body,
        args=(dataset,),
        rounds=1,
    )


def align_fixed_frequency_body(dataset: DatasetEntry) -> None:
    """Align two columns to a fixed frequency."""
    # Grab the cheaper column to get range of times until we can pushdown
    cheaper_column = (
        dataset.dataframe_query_view(index="real_time", contents="/observation/joint_positions")
        .filter_segment_id("ILIAD_sbd7d2c6_2023_12_24_16h_20m_37s")
        .df()
    )

    min_max = cheaper_column.aggregate(
        "rerun_segment_id", [F.min(col("real_time")).alias("min"), F.max(col("real_time")).alias("max")]
    )

    min_time = min_max.to_arrow_table()["min"].to_numpy().flatten()
    max_time = min_max.to_arrow_table()["max"].to_numpy().flatten()
    desired_timestamps = np.arange(min_time[0], max_time[0], np.timedelta64(100, "ms"))  # 10Hz
    fixed_hz = (
        dataset.dataframe_query_view(index="real_time", contents="/observation/joint_positions /camera/ext1/embedding")
        .filter_segment_id("ILIAD_sbd7d2c6_2023_12_24_16h_20m_37s")
        .using_index_values(desired_timestamps)
        # Note if you apply null filter here it is on the source data not the filled data
        # TODO(RR-2769)
        .df()
    )

    # This is the desired product for downstream work
    result = fixed_hz.filter(
        col("/observation/joint_positions:Scalars:scalars").is_not_null(),
        col("/camera/ext1/embedding:embeddings").is_not_null(),
    ).collect()
    assert len(result) > 0


@pytest.mark.benchmark(group="droid")
def test_demonstrate_schema_latency(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Demonstrate schema latency."""
    benchmark.pedantic(
        demonstrate_schema_latency_body,
        args=(dataset,),
        rounds=20,
    )


def demonstrate_schema_latency_body(dataset: DatasetEntry) -> None:
    """Demonstrate schema latency."""
    schema = dataset.schema()
    assert schema is not None


@pytest.mark.benchmark(group="droid")
def test_demonstrate_df_latency(benchmark: BenchmarkFixture, dataset: DatasetEntry) -> None:
    """Demonstrate df latency."""
    # Df calls schema but makes other network calls as well
    # Mostly relevant if we expect to query the cloud for more inner loop work
    qv = dataset.dataframe_query_view(index="real_time", contents="/observation/joint_positions").filter_segment_id(
        "ILIAD_sbd7d2c6_2023_12_24_16h_20m_37s"
    )

    benchmark.pedantic(
        demonstrate_df_latency_body,
        args=(qv,),
        rounds=10,
    )


def demonstrate_df_latency_body(qv: DataframeQueryView) -> None:
    """Demonstrate df latency."""
    # Intentionally no collect because
    # this is what is slow
    qv.df()


@pytest.mark.benchmark(group="droid")
def test_droid_register(
    benchmark: BenchmarkFixture,
    droid_dataset_name: str,
    catalog_client: CatalogClient,
    aws_segments_to_register: list[str],
) -> None:
    """Benchmark dataset registration with manifest-based specification."""
    benchmark.pedantic(
        droid_register,
        args=(droid_dataset_name, catalog_client, aws_segments_to_register),
        rounds=1,
    )


def droid_register(droid_dataset_name: str, catalog_client: CatalogClient, aws_segments_to_register: list[str]) -> None:
    """Benchmark dataset registration with manifest-based specification."""

    droid_dataset_name = f"{uuid.uuid4().hex}-{droid_dataset_name}"
    dataset_handle = catalog_client.create_dataset(droid_dataset_name)

    try:
        task_ids = dataset_handle.register_batch(aws_segments_to_register)
        task_ids.wait(timeout_secs=600)
    finally:
        dataset_handle.delete()
