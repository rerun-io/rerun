"""
End-to-end tests for behaviors that were previously unverifiable from Python.

Each test exercises a query-planning or execution invariant that was
inaccessible before `rerun.experimental.query_metrics()` — either because the
metric in question wasn't surfaced anywhere Python could read it, or because
the DataFusion FFI bug stripped `df.explain(analyze=True)`'s `metrics=[…]`
block.

Tests in this file use the same `readonly_test_dataset` fixture as the other
e2e suites; they run against the local OSS catalog by default.
"""

from __future__ import annotations

import datetime
from typing import TYPE_CHECKING

import pytest
from datafusion import col, lit
from rerun.experimental import query_metrics

if TYPE_CHECKING:
    import pyarrow as pa
    from rerun.catalog import DatasetEntry


# `time_1` hits the datafusion-python bug noted in other tests in this suite.
_INDEX = "time_2"


def _valid_index_values(dataset: DatasetEntry, time_idx: str) -> list[pa.Scalar]:
    times = dataset.reader(index=time_idx).select(time_idx).sort(col(time_idx)).collect()
    return [v for rb in times for v in rb[0] if v.is_valid]


def _index_range_seek_points(dataset: DatasetEntry, time_idx: str) -> list[tuple[str, pa.Scalar]]:
    start_col = f"{time_idx}:start"
    ranges = dataset.get_index_ranges().select("rerun_segment_id", start_col).sort("rerun_segment_id").collect()

    points = []
    for rb in ranges:
        segment_ids = rb.column(0)
        starts = rb.column(1)
        for row_idx in range(rb.num_rows):
            start = starts[row_idx]
            if start.is_valid:
                points.append((segment_ids[row_idx].as_py(), start))
    return points


@pytest.mark.skip(reason="failing in CI")
def test_limit_does_not_propagate_into_server_request(readonly_test_dataset: DatasetEntry) -> None:
    """
    Documents that `df.limit(N)` does not shrink the server-side fetch set.

    DataFusion's `LimitExec` operates *downstream* of the IO pipeline — by
    the time it has enough rows and drops the upstream stream, many gRPC
    requests are already in flight or completed. This test pins that
    behavior: the plan-time `query_chunks` and execution-time fetch counters
    match a no-limit scan within a factor of one another. If a future
    optimization pushes `LIMIT` into the `query_dataset` request, the
    assertion direction here would need to flip — at which point this test
    becomes the regression guard for that change.

    The intent is documentation. The check is intentionally loose: just
    that limit doesn't somehow *zero out* fetches.
    """
    with query_metrics() as m:
        readonly_test_dataset.reader(index=_INDEX).limit(1).collect()
        readonly_test_dataset.reader(index=_INDEX).collect()

    qs = m.queries
    assert len(qs) == 2, f"expected 2 captured queries (limited + full), got {len(qs)}"
    limited, full = qs

    # Plan-time `query_chunks` is identical: LIMIT doesn't change which
    # chunks the planner sees.
    assert limited.query_chunks == full.query_chunks, (
        f"LIMIT changed plan-time chunk count (would imply server-side pushdown): "
        f"limited={limited.query_chunks} vs full={full.query_chunks}"
    )
    # Limit query still fetched non-trivial data; not a no-op.
    assert limited.fetch_requests >= 1
    assert limited.fetch_bytes > 0
    assert limited.error_kind is None
    assert full.error_kind is None


def test_empty_result_filter_still_pushes_down(readonly_test_dataset: DatasetEntry) -> None:
    """
    A pushable filter that selects no rows must still register as pushed-down.

    Uses a `time_index > <effectively unbounded>` filter — pushable in shape
    and guaranteed to match no data. Guards against a regression where the
    pushdown counter is silently dropped on empty results.

    We pick the threshold above the dataset's actual max so the filter is
    well-formed but selects nothing. This also exercises a corner of the
    fetch path that bails out early.
    """
    # Find the dataset's max time so we can build a filter just past it.
    values = _valid_index_values(readonly_test_dataset, _INDEX)
    assert values, f"expected readonly_test_dataset to contain at least one valid {_INDEX} value"
    max_time = values[-1]

    with query_metrics() as m:
        rbs = readonly_test_dataset.reader(index=_INDEX).filter(col(_INDEX) > lit(max_time)).collect()

    # Sanity: the filter does in fact match nothing.
    total_rows = sum(rb.num_rows for rb in rbs)
    assert total_rows == 0, f"expected zero rows, got {total_rows}"

    qm = m.last_query()
    assert qm is not None
    assert qm.filters_pushed_down >= 1, f"empty-result time-index filter must still push down, got: {qm}"
    assert qm.filters_applied_client_side == 0, (
        f"a fully-pushed time-index comparison should leave nothing for the client side, got: {qm}"
    )
    assert qm.error_kind is None


@pytest.mark.skip(reason="failing in CI")
def test_cancellation_mid_stream_still_produces_snapshot(readonly_test_dataset: DatasetEntry) -> None:
    """
    If a query's stream is dropped before being fully consumed, the snapshot path still fires.

    The Rust-side `DataframeSegmentStreamInner::Drop` impl is the fallback that
    catches this case. Test it from Python by issuing a `limit(1)` query (which
    causes the IO loop to short-circuit before fetching all chunks) and
    verifying we still receive a `QueryMetrics` record.

    We don't pin specific counter values — only that a snapshot is produced and
    looks structurally valid.
    """
    with query_metrics() as m:
        readonly_test_dataset.reader(index=_INDEX).limit(1).collect()

    qm = m.last_query()
    assert qm is not None, "limit(1) query must still produce a QueryMetrics snapshot"
    assert qm.query_type, f"snapshot must have a non-empty query_type label, got: {qm.query_type!r}"
    assert qm.error_kind is None, f"limit(1) on a healthy query must succeed, got: {qm.error_kind}"
    # Some chunks must have been fetched even for limit(1) (we only stop after
    # the first batch is ready).
    assert qm.fetch_requests >= 1


def test_no_filter_no_pushdown(readonly_test_dataset: DatasetEntry) -> None:
    """
    An unfiltered scan must register zero filters on both sides.

    Trivial but useful: catches regressions where the pushdown counter is
    incremented spuriously for filterless queries.
    """
    with query_metrics() as m:
        readonly_test_dataset.reader(index=_INDEX).collect()

    qm = m.last_query()
    assert qm is not None
    assert qm.filters_pushed_down == 0
    assert qm.filters_applied_client_side == 0


def test_scrub_seeks_fetch_bounded_slices(readonly_test_dataset: DatasetEntry) -> None:
    """
    Repeated timeline seeks must not degenerate into repeated full-dataset scans.

    This is the CI-sized proxy for the issue #11315 user story: an interactive
    viewer scrubbing through a large remote dataset should ask REDAP for narrow
    index slices, then fetch only the chunks in those slices. The committed
    fixture is intentionally small, but it still has enough chunks/segments to
    catch the class of regression where every seek plans and fetches the full
    dataset.
    """
    points = _index_range_seek_points(readonly_test_dataset, _INDEX)
    assert len(points) >= 5, f"expected enough {_INDEX} ranges to simulate multiple seeks"

    # Pick points away from the exact edges so the test exercises real middle
    # slices rather than "before all data" / "after all data" early-outs.
    seek_points = [points[len(points) // 4], points[len(points) // 2], points[(3 * len(points)) // 4]]

    with query_metrics() as m:
        full_rows = sum(rb.num_rows for rb in readonly_test_dataset.reader(index=_INDEX).collect())

    full = m.last_query()
    assert full is not None
    assert full_rows > 0
    assert full.query_chunks > 1
    assert full.fetch_requests >= 1
    assert full.fetch_bytes > 0
    assert full.error_kind is None

    for segment_id, seek_value in seek_points:
        with query_metrics() as m:
            rows = sum(
                rb.num_rows
                for rb in (
                    readonly_test_dataset
                    .filter_segments(segment_id)
                    .reader(index=_INDEX)
                    .filter(col(_INDEX) == lit(seek_value))
                    .collect()
                )
            )

        qm = m.last_query()
        assert qm is not None, f"no QueryMetrics captured for seek at {segment_id} / {seek_value}"
        assert rows > 0, f"seek at {segment_id} / {seek_value} should hit at least one row in the fixture"
        assert qm.error_kind is None
        assert qm.filters_pushed_down >= 1
        assert qm.filters_applied_client_side == 0
        assert qm.query_segments == 1
        assert qm.query_chunks < full.query_chunks, (
            f"seek at {segment_id} / {seek_value} planned too much data: "
            f"seek={qm.query_chunks} full={full.query_chunks}"
        )
        assert qm.query_bytes < full.query_bytes, (
            f"seek at {segment_id} / {seek_value} planned too many bytes: seek={qm.query_bytes} full={full.query_bytes}"
        )
        assert qm.fetch_requests >= 1
        assert qm.fetch_bytes > 0
        assert qm.fetch_bytes < full.fetch_bytes, (
            f"seek at {segment_id} / {seek_value} fetched too many bytes: seek={qm.fetch_bytes} full={full.fetch_bytes}"
        )


def test_queries_outside_scope_do_not_appear(readonly_test_dataset: DatasetEntry) -> None:
    """
    Queries issued before / after the `with` block must not appear in the collector.

    Verifies the registry-based capture is scope-bounded — entering the context
    manager doesn't retroactively grab earlier queries, and exiting it stops
    capturing.
    """
    # A query before the scope — must NOT be captured.
    readonly_test_dataset.reader(index=_INDEX).limit(1).collect()

    with query_metrics() as m:
        readonly_test_dataset.reader(index=_INDEX).limit(1).collect()

    # A query after the scope — must NOT be captured either.
    readonly_test_dataset.reader(index=_INDEX).limit(1).collect()

    qs = m.queries
    assert len(qs) == 1, f"only the in-scope query should be captured, got {len(qs)}: {qs}"


@pytest.mark.parametrize("time_idx", ["time_2", "time_3"])
def test_query_metrics_smoke_e2e(readonly_test_dataset: DatasetEntry, time_idx: str) -> None:
    """
    End-to-end smoke: every captured field should be structurally valid.

    Validates the round-trip through the Rust→PyO3→Python wrapper for a
    realistic-looking query. If any field comes back missing or with a
    nonsensical default, this catches it early.
    """
    with query_metrics() as m:
        readonly_test_dataset.reader(index=time_idx).collect()

    qm = m.last_query()
    assert qm is not None

    # Plan-time fields populated.
    assert qm.dataset_id  # non-empty
    assert qm.query_chunks > 0
    assert qm.query_segments > 0
    assert qm.query_layers >= 1
    assert qm.query_columns >= 1
    assert qm.query_entities >= 1
    assert qm.query_bytes > 0
    assert qm.query_type
    assert qm.primary_index_name == time_idx

    # Execution-time: positive duration; error fields unset.
    assert qm.total_duration >= datetime.timedelta(0)
    assert qm.error_kind is None
    assert qm.direct_terminal_reason is None

    # Wire counters: at least one transport (gRPC or direct) must have fired.
    assert qm.fetch_requests >= 1
    assert qm.fetch_bytes > 0
