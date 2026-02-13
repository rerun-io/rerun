from __future__ import annotations

from typing import TYPE_CHECKING

from datafusion import Expr, col, functions as f, lit

if TYPE_CHECKING:
    import pyarrow as pa
    from rerun.catalog import CatalogClient, DatasetEntry


def test_df_filters(catalog_client: CatalogClient, readonly_test_dataset: DatasetEntry) -> None:
    """
    Tests filter pushdown correctness.

    These tests will verify that our push-down filtering returns the exact same results
    as without push-down. It does this by first collecting record batches without any filters
    and turning them into an in-memory table. Then we run the same filters on both the in-memory
    table and the dataset to demonstrate we get exactly the same results.

    This test does *not* guarantee that the push-down filters are being applied in the gRPC
    requests.
    """

    all_segments = (
        readonly_test_dataset.reader(index=None).select("rerun_segment_id").sort(col("rerun_segment_id")).collect()
    )
    all_segments = [v for rb in all_segments for v in rb[0]]

    def find_time_boundaries(time_index: str, segment: pa.Scalar) -> list[pa.Scalar]:
        """Find four times: start, middle third, upper third, stop."""
        rbs = (
            readonly_test_dataset.reader(index=time_index)
            .filter(col("rerun_segment_id") == segment)
            .select(time_index)
            .sort(col(time_index))
            .collect()
        )
        values = [v for rb in rbs for v in rb[0]]
        num_vals = len(values)
        return [values[0], values[num_vals // 3], values[2 * num_vals // 3], values[num_vals - 1]]

    def generate_tests(time_index: str, segments: list[pa.Scalar]) -> list[Expr]:
        """Create a set of filters for testing."""
        seg1_times = find_time_boundaries(time_index, segments[0])
        seg2_times = find_time_boundaries(time_index, segments[1])
        s1_min = lit(seg1_times[0])
        s1_lower = lit(seg1_times[1])
        s1_upper = lit(seg1_times[2])
        s1_max = lit(seg1_times[3])
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
            ((col("rerun_segment_id") == segments[0]) & (col(time_index) > s1_lower))
            | ((col("rerun_segment_id") == segments[1]) & (col(time_index) < s2_lower)),
            # Edge cases
            col(time_index) == s1_lower,  # Exact match, multiple segments
            # Non-parsable cases should have no impact
            f.substring(col("rerun_segment_id"), lit(2), lit(3)) == "some_value",
        ]

    # Cannot run "time_1" due to https://github.com/apache/datafusion-python/pull/1319
    for time_idx in ["time_2", "time_3"]:
        all_tests = generate_tests(time_idx, all_segments)

        # Collect all data without any filtering and store in memory
        # so that we can have guarantees that our push-down filters
        # do not impact the results.
        full_data_batches = readonly_test_dataset.reader(index=time_idx).collect()
        catalog_client.ctx.register_record_batches(time_idx, [full_data_batches])
        full_data = catalog_client.ctx.table(time_idx)

        for test_filter in all_tests:
            # We must sort to guarantee the output ordering
            results = readonly_test_dataset.reader(index=time_idx).filter(test_filter).sort(col("log_time")).collect()
            expected = full_data.filter(test_filter).sort(col("log_time")).collect()

            assert results == expected
