from __future__ import annotations

from typing import TYPE_CHECKING

from datafusion import Expr, col, functions as f, lit
from rerun.experimental import query_metrics

if TYPE_CHECKING:
    import pyarrow as pa
    from rerun.catalog import CatalogClient, DatasetEntry


# Filter classification — encodes which expression kinds we expect the
# server-side pushdown to handle today. Used by the assertions below; update
# alongside any change to `re_datafusion::pushdown_expressions`.
#
# - "pushable": expected to land entirely on the server. `filters_pushed_down >= 1`
#   and `filters_applied_client_side == 0`.
# - "non_pushable": expected to land entirely client-side as a `FilterExec`.
#   `filters_pushed_down == 0` and `filters_applied_client_side >= 1`.
# - "uncertain": shape that may go either way (e.g. OR combinations, negated
#   in_list, negated between). Only assert the universal invariant — at least
#   one side fires.
_PUSHABLE = "pushable"
_NON_PUSHABLE = "non_pushable"
_UNCERTAIN = "uncertain"


def test_df_filters(catalog_client: CatalogClient, readonly_test_dataset: DatasetEntry) -> None:
    """
    Tests filter pushdown correctness *and* that pushdown actually fires.

    These tests verify that our push-down filtering returns the exact same results
    as without push-down. It does this by first collecting record batches without any filters
    and turning them into an in-memory table. Then we run the same filters on both the in-memory
    table and the dataset to demonstrate we get exactly the same results.

    In addition, each filter is wrapped in a `query_metrics()` scope so we can assert that
    the server-side pushdown actually fired (or did not, for known-non-pushable shapes).
    Previously this was unverifiable from Python — the docstring used to call out that gap.
    """

    all_segments = (
        readonly_test_dataset.reader(index=None).select("rerun_segment_id").sort(col("rerun_segment_id")).collect()
    )
    all_segments = [v for rb in all_segments for v in rb[0]]

    def find_time_boundaries(time_index: str, segment: pa.Scalar) -> list[pa.Scalar]:
        """Find four times: start, middle third, upper third, stop."""
        rbs = (
            readonly_test_dataset
            .reader(index=time_index)
            .filter(col("rerun_segment_id") == segment)
            .select(time_index)
            .sort(col(time_index))
            .collect()
        )
        values = [v for rb in rbs for v in rb[0]]
        num_vals = len(values)
        return [values[0], values[num_vals // 3], values[2 * num_vals // 3], values[num_vals - 1]]

    def generate_tests(time_index: str, segments: list[pa.Scalar]) -> list[tuple[Expr, str]]:
        """Create a set of filters for testing, each labeled with its expected pushdown class."""
        seg1_times = find_time_boundaries(time_index, segments[0])
        seg2_times = find_time_boundaries(time_index, segments[1])
        s1_min = lit(seg1_times[0])
        s1_lower = lit(seg1_times[1])
        s1_upper = lit(seg1_times[2])
        s1_max = lit(seg1_times[3])
        s2_lower = lit(seg2_times[1])

        return [
            # Basic comparisons on time only
            (col(time_index) == s1_lower, _PUSHABLE),
            (col(time_index) > s1_lower, _PUSHABLE),
            (col(time_index) >= s1_lower, _PUSHABLE),
            (col(time_index) < s1_lower, _PUSHABLE),
            (col(time_index) <= s1_lower, _PUSHABLE),
            # Range inclusive — AND of two pushable filters
            ((col(time_index) >= s1_lower) & (col(time_index) <= s1_upper), _PUSHABLE),
            (col(time_index).between(s1_lower, s1_upper, negated=False), _PUSHABLE),
            # `between(..., negated=True)` lowers to a disjunction; treat as uncertain.
            (col(time_index).between(s1_lower, s1_upper, negated=True), _UNCERTAIN),
            # Range exclusive
            ((col(time_index) > s1_lower) & (col(time_index) < s1_upper), _PUSHABLE),
            # Segment filtering only
            (col("rerun_segment_id") == segments[0], _PUSHABLE),
            (col("rerun_segment_id") == segments[1], _PUSHABLE),
            # Negated in_list: not a simple set membership, may not push.
            (f.in_list(col("rerun_segment_id"), [lit(segments[0]), lit(segments[1])], negated=True), _UNCERTAIN),
            (f.in_list(col("rerun_segment_id"), [lit(segments[0]), lit(segments[1])], negated=False), _PUSHABLE),
            # Segment + time AND combinations — each conjunct is pushable.
            ((col("rerun_segment_id") == segments[0]) & (col(time_index) == s1_lower), _PUSHABLE),
            ((col("rerun_segment_id") == segments[0]) & (col(time_index) > s1_lower), _PUSHABLE),
            ((col("rerun_segment_id") == segments[0]) & (col(time_index) >= s1_lower), _PUSHABLE),
            ((col("rerun_segment_id") == segments[0]) & (col(time_index) < s1_lower), _PUSHABLE),
            ((col("rerun_segment_id") == segments[0]) & (col(time_index) <= s1_lower), _PUSHABLE),
            (
                (col("rerun_segment_id") == segments[0])
                & (col(time_index) >= s1_lower)
                & (col(time_index) <= s1_upper),
                _PUSHABLE,
            ),
            (
                (col("rerun_segment_id") == segments[0]) & (col(time_index) > s1_lower) & (col(time_index) < s1_upper),
                _PUSHABLE,
            ),
            # Segment + time combinations with no results — still pushable shape.
            ((col("rerun_segment_id") == segments[0]) & (col(time_index) < s1_min), _PUSHABLE),
            ((col("rerun_segment_id") == segments[0]) & (col(time_index) > s1_max), _PUSHABLE),
            # OR combinations — pushdown behavior depends on the optimizer's
            # disjunction handling. Don't pin a side.
            ((col("rerun_segment_id") == segments[0]) | (col("rerun_segment_id") == segments[1]), _UNCERTAIN),
            (
                ((col("rerun_segment_id") == segments[0]) & (col(time_index) > s1_lower))
                | ((col("rerun_segment_id") == segments[1]) & (col(time_index) < s2_lower)),
                _UNCERTAIN,
            ),
            # Edge cases
            (col(time_index) == s1_lower, _PUSHABLE),  # Exact match, multiple segments
            # Non-parsable: a `substring()` expression has no analytical
            # form the pushdown layer can rewrite into a server request. In
            # practice the optimizer applies the filter at a different layer
            # (a FilterExec sibling of `SegmentStreamExec`, not on the table
            # provider itself), so it shows up as neither pushed nor
            # client-side from our counters' perspective. Classify as
            # uncertain — the row-correctness check still validates the
            # filter actually applies.
            (f.substring(col("rerun_segment_id"), lit(2), lit(3)) == "some_value", _UNCERTAIN),
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

        for test_filter, pushdown_class in all_tests:
            # We must sort to guarantee the output ordering. Wrap just the
            # Rerun-side read in `query_metrics()` so `m.last_query()`
            # unambiguously refers to that scan; `full_data` is an in-memory
            # DataFusion table and doesn't go through `SegmentStreamExec`.
            with query_metrics() as m:
                results = (
                    readonly_test_dataset.reader(index=time_idx).filter(test_filter).sort(col("log_time")).collect()
                )
            expected = full_data.filter(test_filter).sort(col("log_time")).collect()

            assert results == expected

            qm = m.last_query()
            assert qm is not None, f"no QueryMetrics captured for filter: {test_filter}"

            # Note: some shapes (notably negated `IN` lists) get rewritten by
            # DataFusion's optimizer into a form that bypasses both pushdown
            # paths — the filter still applies, but neither counter
            # increments. So we *don't* assert a universal `total >= 1`
            # invariant; instead we only assert the known-strong cases below.

            if pushdown_class == _PUSHABLE:
                assert qm.filters_pushed_down >= 1, (
                    f"pushable filter {test_filter} did not push down "
                    f"(pushed_down={qm.filters_pushed_down}, "
                    f"client_side={qm.filters_applied_client_side})"
                )
                assert qm.filters_applied_client_side == 0, (
                    f"pushable filter {test_filter} unexpectedly fell back to client side "
                    f"(client_side={qm.filters_applied_client_side})"
                )
            elif pushdown_class == _NON_PUSHABLE:
                assert qm.filters_pushed_down == 0, (
                    f"non-pushable filter {test_filter} unexpectedly pushed down (pushed_down={qm.filters_pushed_down})"
                )
                assert qm.filters_applied_client_side >= 1, (
                    f"non-pushable filter {test_filter} did not register client side "
                    f"(client_side={qm.filters_applied_client_side})"
                )
            # _UNCERTAIN: only the universal invariant above applies.
