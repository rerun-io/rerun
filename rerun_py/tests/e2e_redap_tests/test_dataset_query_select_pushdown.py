"""
End-to-end tests for the entity-path select-pushdown optimization.

When a dataframe query projects a subset of component columns, the server request is narrowed
to fetch chunks only for entity paths referenced by the projection + filters. This is gated on
`SparseFillStrategy::None`: under `fill_latest_at=True` the optimization is skipped because
excluded entities' timestamps would otherwise produce filled rows the caller expects.

Observable side effect under `SparseFillStrategy::None`: rows where every selected component
column would have been null are no longer emitted, because the chunks that would have produced
those index rows are never fetched.

Strategy: each scenario builds an "expected" result by issuing a baseline query that selects
**all** entity columns (so narrowing keeps every entity path) and then filters/projects post-hoc
to the shape the narrowed query is supposed to produce. The two should be equal.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from datafusion import col
from rerun.experimental import query_metrics

if TYPE_CHECKING:
    from datafusion import DataFrame
    from rerun.catalog import DatasetEntry
    from syrupy import SnapshotAssertion


OBJ1 = "/obj1:Points3D:positions"
OBJ2 = "/obj2:Points3D:positions"
OBJ3 = "/obj3:Points3D:positions"


def _materialize(df: DataFrame) -> pa.Table:
    """Materialize a DataFrame to a single-chunk Arrow table for chunking-insensitive comparison."""
    return pa.Table.from_batches(df.collect()).combine_chunks()


def _full_query(dataset: DatasetEntry, time_idx: str) -> DataFrame:
    """
    Baseline reader that selects all three /obj* entity columns.

    Selecting every entity column means the narrowing logic — even when it fires — keeps every
    entity path in the request, so the result row set matches the pre-optimization behavior.
    """
    return (
        dataset
        .reader(index=time_idx)
        .select("rerun_segment_id", time_idx, OBJ1, OBJ2, OBJ3)
        .sort("rerun_segment_id", time_idx)
    )


@pytest.mark.parametrize("time_idx", ["time_1", "time_2", "time_3"])
def test_narrowing_drops_all_null_rows_single_entity(readonly_test_dataset: DatasetEntry, time_idx: str) -> None:
    """
    SELECT one entity column — narrowing drops rows where that column would be null.

    Asserts both row-correctness *and* that the narrowing optimization actually fired —
    previously the latter was only observable in `EXPLAIN ANALYZE` output.
    """
    # Build both queries inside the `query_metrics()` scope: collectors are
    # bound at `reader()` time, so readers built outside the scope are not
    # captured. Order: narrowed first (via `_materialize`), then the
    # `_full_query`-derived expected.
    with query_metrics() as m:
        narrowed = (
            readonly_test_dataset
            .reader(index=time_idx)
            .select("rerun_segment_id", time_idx, OBJ1)
            .sort("rerun_segment_id", time_idx)
        )

        expected = (
            _full_query(readonly_test_dataset, time_idx)
            .filter(col(OBJ1).is_not_null())
            .select("rerun_segment_id", time_idx, OBJ1)
            .sort("rerun_segment_id", time_idx)
        )

        narrowed_tbl = _materialize(narrowed)
        expected_tbl = _materialize(expected)

    assert narrowed_tbl == expected_tbl

    qs = m.queries
    assert len(qs) == 2, f"expected 2 captured queries (narrowed + baseline), got {len(qs)}"
    nar, _base = qs
    # Core claim: a single-entity SELECT triggers narrowing. This was
    # previously unverifiable from Python — only inspectable in the
    # `EXPLAIN ANALYZE` output.
    #
    # We deliberately *don't* compare `nar.query_chunks` to the baseline's:
    # the `_full_query` baseline also triggers narrowing (the dataset has
    # more entities than its SELECT references), and the OR'd
    # `IS NOT NULL` filter it applies pushes server-side, so the two
    # ultimately fetch comparable chunk counts. The flag itself is the
    # cleanest assertion.
    assert nar.entity_path_narrowing_applied is True, (
        f"single-entity SELECT must trigger entity-path narrowing, got {nar}"
    )


@pytest.mark.parametrize("time_idx", ["time_1", "time_2", "time_3"])
def test_narrowing_drops_all_null_rows_two_entities(readonly_test_dataset: DatasetEntry, time_idx: str) -> None:
    """SELECT two entity columns — narrowing drops rows where both would be null."""
    with query_metrics() as m:
        narrowed = (
            readonly_test_dataset
            .reader(index=time_idx)
            .select("rerun_segment_id", time_idx, OBJ1, OBJ2)
            .sort("rerun_segment_id", time_idx)
        )

        expected = (
            _full_query(readonly_test_dataset, time_idx)
            .filter(col(OBJ1).is_not_null() | col(OBJ2).is_not_null())
            .select("rerun_segment_id", time_idx, OBJ1, OBJ2)
            .sort("rerun_segment_id", time_idx)
        )

        narrowed_tbl = _materialize(narrowed)
        expected_tbl = _materialize(expected)

    assert narrowed_tbl == expected_tbl

    qs = m.queries
    assert len(qs) == 2, f"expected 2 captured queries (narrowed + baseline), got {len(qs)}"
    nar, _base = qs
    # See `test_narrowing_drops_all_null_rows_single_entity` for why we only
    # assert the flag here, not chunk-count ordinality.
    assert nar.entity_path_narrowing_applied is True


@pytest.mark.parametrize("time_idx", ["time_1", "time_2", "time_3"])
def test_fill_latest_at_disables_narrowing(readonly_test_dataset: DatasetEntry, time_idx: str) -> None:
    """
    With `fill_latest_at=True` (LatestAtGlobal) narrowing is gated off — rows are preserved.

    Under LatestAtGlobal, excluded entities' timestamps would generate rows filled with the
    latest values, so the optimization must not drop them. We assert the narrowed query's row
    count equals the baseline's, and the projected /obj1 column matches.

    With `query_metrics()` we now also directly verify the gating logic fired — previously
    we could only infer it from the row-count parity.
    """
    with query_metrics() as m:
        narrowed_fill = (
            readonly_test_dataset
            .reader(index=time_idx, fill_latest_at=True)
            .select("rerun_segment_id", time_idx, OBJ1)
            .sort("rerun_segment_id", time_idx)
        )

        full_fill = (
            readonly_test_dataset
            .reader(index=time_idx, fill_latest_at=True)
            .select("rerun_segment_id", time_idx, OBJ1, OBJ2, OBJ3)
            .sort("rerun_segment_id", time_idx)
        )

        narrowed_table = _materialize(narrowed_fill)
        full_table = _materialize(full_fill)

    # Narrowing is skipped → row count matches the unprojected baseline.
    assert narrowed_table.num_rows == full_table.num_rows
    # Project full_table down to the same columns post-hoc rather than running
    # another DataFrame query — re-materializing a DataFrame whose provider
    # was bound inside the scope would emit a 3rd snapshot to the collector.
    assert narrowed_table == full_table.select(["rerun_segment_id", time_idx, OBJ1])

    qs = m.queries
    assert len(qs) == 2, f"expected 2 captured queries, got {len(qs)}"
    nar, full = qs
    # The whole point of this test: narrowing is gated off by fill_latest_at=True.
    assert nar.entity_path_narrowing_applied is False, (
        f"fill_latest_at=True must disable narrowing, but it fired on a single-entity SELECT: {nar}"
    )
    assert full.entity_path_narrowing_applied is False
    # Both queries fetch the same data when narrowing is gated off.
    assert nar.query_chunks == full.query_chunks, (
        f"with narrowing gated off, both queries should fetch the same chunks: "
        f"nar={nar.query_chunks} vs full={full.query_chunks}"
    )


@pytest.mark.parametrize("time_idx", ["time_1", "time_2", "time_3"])
def test_filter_on_other_entity_expands_fetch_set(readonly_test_dataset: DatasetEntry, time_idx: str) -> None:
    """
    A filter referencing /obj2 forces /obj2 chunks to be fetched even when only /obj1 is selected.

    This is the "case where we cannot filter out" — chunks for /obj2 are needed to evaluate the
    filter, so /obj2's index timestamps appear in the underlying scan. Rows are then dropped by
    the explicit `IS NOT NULL` filter, but rows where /obj1 has data and /obj2 also has data at
    the same timestamp survive (with /obj1's value alongside the non-null /obj2).
    """
    narrowed = (
        readonly_test_dataset
        .reader(index=time_idx)
        .filter(col(OBJ2).is_not_null())
        .select("rerun_segment_id", time_idx, OBJ1)
        .sort("rerun_segment_id", time_idx)
    )

    expected = (
        _full_query(readonly_test_dataset, time_idx)
        .filter(col(OBJ2).is_not_null())
        .select("rerun_segment_id", time_idx, OBJ1)
        .sort("rerun_segment_id", time_idx)
    )

    assert _materialize(narrowed) == _materialize(expected)


@pytest.mark.parametrize("time_idx", ["time_1", "time_2", "time_3"])
def test_filter_contents_view_narrowing_intersection(readonly_test_dataset: DatasetEntry, time_idx: str) -> None:
    """
    View-level entity filter intersects with column-projection narrowing.

    `filter_contents(["/obj1/**", "/obj2/**"])` restricts the view to /obj1 and /obj2.
    Selecting only /obj1 then narrows further to {/obj1}. Rows from /obj3-only timestamps are
    absent (excluded by the view), and rows where /obj1 is null are absent (excluded by
    narrowing). The baseline must apply both restrictions explicitly.
    """
    view = readonly_test_dataset.filter_contents(["/obj1/**", "/obj2/**"])

    narrowed = view.reader(index=time_idx).select("rerun_segment_id", time_idx, OBJ1).sort("rerun_segment_id", time_idx)

    # Baseline within the same view (still excludes /obj3) — selecting both view-allowed
    # entity columns means narrowing keeps both, so this is the full row set for the view.
    full_view = (
        view.reader(index=time_idx).select("rerun_segment_id", time_idx, OBJ1, OBJ2).sort("rerun_segment_id", time_idx)
    )

    expected = (
        full_view
        .filter(col(OBJ1).is_not_null())
        .select("rerun_segment_id", time_idx, OBJ1)
        .sort("rerun_segment_id", time_idx)
    )

    assert _materialize(narrowed) == _materialize(expected)


@pytest.mark.parametrize(
    "time_idx", ["time_2", "time_3"]
)  # time_1 hits datafusion-python bug — see test_dataset_query_filter.py
def test_filter_on_index_column_does_not_expand_fetch_set(readonly_test_dataset: DatasetEntry, time_idx: str) -> None:
    """
    A filter on the time index column does not expand the fetch set.

    Time index columns have no entity-path metadata, so referencing one in a filter doesn't add
    any entity to the projected set. Narrowing still drops all-null-/obj1 rows that pass the
    time filter.

    With `query_metrics()` we additionally assert the fetch set didn't grow — comparing the
    time-filtered narrowed query to a baseline narrowed query without the time filter.
    """
    # Pick a threshold from the dataset itself so the filter is meaningful regardless of which
    # time index we're parametrized over.
    times = readonly_test_dataset.reader(index=time_idx).select(time_idx).sort(time_idx).collect()
    values = [v for rb in times for v in rb[0] if v.is_valid]
    threshold = values[len(values) // 3]

    # Build the three readers inside the scope so each gets the active
    # collector bound at `reader()` time.
    with query_metrics() as m:
        narrowed = (
            readonly_test_dataset
            .reader(index=time_idx)
            .filter(col(time_idx) > threshold)
            .select("rerun_segment_id", time_idx, OBJ1)
            .sort("rerun_segment_id", time_idx)
        )

        expected = (
            _full_query(readonly_test_dataset, time_idx)
            .filter(col(time_idx) > threshold)
            .filter(col(OBJ1).is_not_null())  # explicit — narrowing drops these implicitly
            .select("rerun_segment_id", time_idx, OBJ1)
            .sort("rerun_segment_id", time_idx)
        )

        # Baseline: same narrowed SELECT without the time-index filter. Used to
        # check that adding the filter doesn't expand the entity fetch set.
        narrowed_no_filter = (
            readonly_test_dataset
            .reader(index=time_idx)
            .select("rerun_segment_id", time_idx, OBJ1)
            .sort("rerun_segment_id", time_idx)
        )

        narrowed_tbl = _materialize(narrowed)
        expected_tbl = _materialize(expected)
        baseline_tbl = _materialize(narrowed_no_filter)

    assert narrowed_tbl == expected_tbl
    # Baseline materialization just keeps things consistent with the
    # `query_metrics()` scope; we use its metrics, not its rows.
    _ = baseline_tbl

    qs = m.queries
    assert len(qs) == 3, f"expected 3 captured queries, got {len(qs)}"
    nar, _exp, baseline_qm = qs
    assert nar.entity_path_narrowing_applied is True, f"narrowing must fire on single-entity SELECT: {nar}"
    assert nar.query_chunks <= baseline_qm.query_chunks, (
        f"time-index filter must NOT expand the fetch set: "
        f"with-filter={nar.query_chunks} vs without-filter={baseline_qm.query_chunks}"
    )


# -----------------------------------------------------------------------------
# Snapshot tests (regression guard for exact output against the committed .rrd fixture).
# -----------------------------------------------------------------------------


def test_narrowed_select_snapshot(readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion) -> None:
    """Pin the exact narrowed output for SELECT /obj1 — guards against accidental row-count drift."""
    df = (
        readonly_test_dataset
        .reader(index="time_1")
        .select("rerun_segment_id", "time_1", OBJ1)
        .sort("rerun_segment_id", "time_1")
    )

    df_schema = df.schema()
    for batch in df.collect():
        assert batch.schema.equals(df_schema, check_metadata=True)

    assert str(df) == snapshot
    assert str(pa.table(df)) == snapshot


def test_fill_latest_at_no_narrow_snapshot(readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion) -> None:
    """Pin the exact output when narrowing is gated off by `fill_latest_at=True`."""
    df = (
        readonly_test_dataset
        .reader(index="time_1", fill_latest_at=True)
        .select("rerun_segment_id", "time_1", OBJ1)
        .sort("rerun_segment_id", "time_1")
    )

    df_schema = df.schema()
    for batch in df.collect():
        assert batch.schema.equals(df_schema, check_metadata=True)

    assert str(df) == snapshot
    assert str(pa.table(df)) == snapshot
