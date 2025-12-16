from __future__ import annotations

import datetime
from typing import TYPE_CHECKING

import pyarrow as pa
from datafusion import col, functions as f, lit, Expr

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry


def test_df_filters(catalog_client: CatalogClient, readonly_test_dataset: DatasetEntry) -> None:
    """
    Tests count() on a dataframe which ensures we collect empty batches properly.

    See issue https://github.com/rerun-io/rerun/issues/10894 for additional context.
    """

    all_segments = (
        readonly_test_dataset
        .dataframe_query_view(index=None, contents="/**")
        .df()
        .select("rerun_segment_id")
        .sort(col("rerun_segment_id"))
        .collect()
    )
    all_segments = [v for rb in all_segments for v in rb[0]]

    def find_time_boundaries(time_index: str, segment: pa.Scalar) -> list[pa.Scalar]:
        """Find four times: start, middle third, upper third, stop"""
        rbs = (
            readonly_test_dataset
            .dataframe_query_view(index=time_index, contents="/**")
            .df()
            .filter(col("rerun_segment_id") == segment)
            .select(time_index)
            .sort(col(time_index))
            .collect()
        )
        values = [v for rb in rbs for v in rb[0]]
        num_vals = len(values)
        return [values[0], values[num_vals//3], values[2*num_vals//3], values[num_vals-1]]

    def generate_tests(time_index: str, segments: list[pa.Scalar]) -> list[Expr]:
        seg1_times = find_time_boundaries(time_index, segments[0])
        seg2_times = find_time_boundaries(time_index, segments[1])
        s1_min =  lit(seg1_times[0])
        s1_lower = lit(seg1_times[1])
        s1_upper = lit(seg1_times[2])
        s1_max =  lit(seg1_times[3])
        s2_lower = lit(seg2_times[1])

        return [
            # Basic comparisons on time only
            col(time_index) == s1_lower,
            col(time_index) > s1_lower,
            col(time_index) >= s1_lower,
            col(time_index) < s1_lower,
            col(time_index) <= s1_lower,
            # Range inclusive
            (col(time_index) >= s1_lower) & (col(time_index) <= s1_upper),
            col(time_index).between(s1_lower, s1_upper, negated=False),
            col(time_index).between(s1_lower, s1_upper, negated=True),
            # Range exclusive
            (col(time_index) > s1_lower) & (col(time_index) < s1_upper),
            # Segment filtering only
            col("rerun_segment_id") == segments[0],
            col("rerun_segment_id") == segments[1],
            f.in_list(col("rerun_segment_id"), [lit(segments[0]), lit(segments[1])], negated=True),
            f.in_list(col("rerun_segment_id"), [lit(segments[0]), lit(segments[1])], negated=False),
            # Segment + time combinations
            (col("rerun_segment_id") == segments[0]) & (col(time_index) == s1_lower),
            (col("rerun_segment_id") == segments[0]) & (col(time_index) > s1_lower),
            (col("rerun_segment_id") == segments[0]) & (col(time_index) >= s1_lower),
            (col("rerun_segment_id") == segments[0]) & (col(time_index) < s1_lower),
            (col("rerun_segment_id") == segments[0]) & (col(time_index) <= s1_lower),
            (col("rerun_segment_id") == segments[0]) & (col(time_index) >= s1_lower) & (col(time_index) <= s1_upper),
            (col("rerun_segment_id") == segments[0]) & (col(time_index) > s1_lower) & (col(time_index) < s1_upper),
            # Segment + time combinations with no results
            (col("rerun_segment_id") == segments[0]) & (col(time_index) < s1_min),
            (col("rerun_segment_id") == segments[0]) & (col(time_index) > s1_max),
            # OR combinations
            (col("rerun_segment_id") == segments[0]) | (col("rerun_segment_id") == segments[1]),
            ((col("rerun_segment_id") == segments[0]) & (col(time_index) > s1_lower)) | ((col("rerun_segment_id") == segments[1]) & (col(time_index) < s2_lower)),
            # Edge cases
            col(time_index) == s1_lower, # Exact match, multiple segments
        ]

    # Cannot run "time_1" due to https://github.com/apache/datafusion-python/pull/1319
    for time_idx in ["time_2", "time_3"]:
        all_tests = generate_tests(time_idx, all_segments)

        # Collect all data without any filtering and store in memory
        # so that we can have guarantees that our push-down filters
        # do not impact the results.
        full_data = readonly_test_dataset.dataframe_query_view(index=time_idx, contents="/**").df().collect()
        catalog_client.ctx.register_record_batches(time_idx, [full_data])
        full_data = catalog_client.ctx.table(time_idx)

        for test_filter in all_tests:
            # We must sort to guarantee the output ordering
            results = readonly_test_dataset.dataframe_query_view(index=time_idx, contents="/**").df().filter(test_filter).sort(col("log_time")).collect()
            expected = full_data.filter(test_filter).sort(col("log_time")).collect()

            assert results == expected
