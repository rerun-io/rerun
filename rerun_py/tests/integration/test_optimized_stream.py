"""
Comparison tests for the new optimizer engine (`_optimized_stream`) against the legacy engine.

The fixture is shaped so that the first-fit sweep packs as well as the legacy election, and so that
the byte-metric gap between the file path (IPC payload size) and the in-memory path (heap size)
never moves a bin boundary.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
import rerun as rr
from rerun.chunk import ChunkStore, LazyStore, OptimizationProfile, RrdReader

from .util import normalized_fields, row_multiset

if TYPE_CHECKING:
    from pathlib import Path

    import datafusion
    from syrupy.assertion import SnapshotAssertion

APP_ID = "rerun_example_optimized_stream"
RECORDING_ID = "optimized-stream-rec-id"

TARGET_BYTES = 2 * 1024 * 1024

# Every row is a fixed-size list of 128 float32: 512 bytes of payload, dominating both byte
# metrics (IPC payload size on the file path, heap size on the in-memory path).
FLOATS_PER_ROW = 128

# The uniform entity: 18 chunks of 600 rows (~315 KiB each) pack 6 per bin at the 2 MiB target,
# away from an exact divisor (a chunk in (~293 KiB, ~341 KiB] gives 6 per bin under either byte
# metric). 3 600 rows per bin stays well under even the legacy engine's stricter 8 192 unsorted-row guard,
# so packing depends on bytes alone.
UNIFORM_CHUNKS = 18
UNIFORM_ROWS = 600
UNIFORM_BINS = 3

# The out-of-order entity: six "big" (~1.18 MiB) and six "small" (~0.69 MiB) chunks, alternating
# in time but written to the file grouped by size. In time order, first-fit packs big+small pairs
# (~1.87 MiB) into 6 bins — the byte lower bound, unbeatable because no bin holds three chunks
# (3 smalls ≈ 2.07 MiB > 2 MiB with row overhead). In file order it packs
# `[big] * 5, [big, small], [small, small] * 2, [small]`: 9 bins.
BIG_ROWS = 2300
SMALL_ROWS = 1350

# The oversized entity: one chunk (~3.07 MiB) beyond the byte target, forcing a split.
OVERSIZED_ROWS = 6000


def _column(num_rows: int, seed: int) -> pa.FixedSizeListArray:
    """One component column: `num_rows` rows of 128 float32 each. Deterministic, NaN-free."""
    values = pa.array(
        [float((seed * 31 + i) % 100_000) for i in range(num_rows * FLOATS_PER_ROW)],
        type=pa.float32(),
    )
    return pa.FixedSizeListArray.from_arrays(values, FLOATS_PER_ROW)


def _send(rec: rr.RecordingStream, entity: str, ticks: range, seed: int) -> None:
    """One `send_columns` call — one intended chunk — on the `tick` timeline."""
    rec.send_columns(
        entity,
        indexes=[rr.TimeColumn("tick", sequence=list(ticks))],
        columns=rr.AnyValues.columns(data=_column(len(ticks), seed)),
    )


@pytest.fixture(scope="session")
def input_rrd_path(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Session-scoped RRD shaped to exercise the planner; one `send_columns` call per chunk."""
    rrd_path = tmp_path_factory.mktemp("optimizer") / "test.rrd"

    with rr.RecordingStream(APP_ID, recording_id=RECORDING_ID) as rec:
        rec.save(rrd_path)

        # Many mergeable chunks of one uniform size, written in time order.
        for i in range(UNIFORM_CHUNKS):
            _send(rec, "/uniform", range(i * 1000, i * 1000 + UNIFORM_ROWS), seed=i)

        # One static chunk: no index columns means static.
        rec.send_columns("/static", indexes=[], columns=rr.TextLog.columns(text=["v1"]))

        # One entity on two timelines, as three small chunks that merge into one output.
        for i in range(3):
            ticks = range(100_000 + i * 100, 100_000 + i * 100 + 50)
            rec.send_columns(
                "/two_timelines",
                indexes=[
                    rr.TimeColumn("tick", sequence=list(ticks)),
                    rr.TimeColumn("tock", sequence=list(range(i * 100, i * 100 + 50))),
                ],
                columns=rr.AnyValues.columns(data=_column(50, seed=100 + i)),
            )

        # One entity logged out of time order: bigs at even time slots, smalls at odd ones, but
        # written to the file grouped by size.
        def slot(j: int) -> int:
            return 200_000 + j * 10_000

        for j in range(0, 12, 2):
            _send(rec, "/out_of_order", range(slot(j), slot(j) + BIG_ROWS), seed=200 + j)
        for j in range(1, 12, 2):
            _send(rec, "/out_of_order", range(slot(j), slot(j) + SMALL_ROWS), seed=200 + j)

        # One oversized chunk beyond the byte target, forcing a split.
        _send(rec, "/oversized", range(500_000, 500_000 + OVERSIZED_ROWS), seed=999)

    return rrd_path


@pytest.fixture(scope="module")
def unoptimized(input_rrd_path: Path) -> ChunkStore:
    """The input chunks, layout preserved. Doubles as the data baseline, through `reader()`."""
    return ChunkStore.from_chunks(list(RrdReader(input_rrd_path).stream()))


@pytest.fixture(scope="module")
def legacy_optimized(input_rrd_path: Path) -> ChunkStore:
    """
    Legacy engine output.

    Object-store thresholds; video passes disabled to pair like with like (the new engine is
    video-unaware).
    """
    return (
        RrdReader(input_rrd_path)
        .stream()
        .collect(
            optimize=OptimizationProfile(
                max_bytes=TARGET_BYTES,
                max_rows=65_536,
                max_rows_if_unsorted=8_192,
                extra_passes=50,
                gop_batching=False,
                split_size_ratio=None,
            )
        )
    )


@pytest.fixture(scope="module")
def optimized_timeline_order(input_rrd_path: Path) -> ChunkStore:
    """New engine output, time-ordered sweep — the legacy election is also time-adjacency driven."""
    return ChunkStore.from_chunks(list(RrdReader(input_rrd_path).store()._optimized_stream(target_timeline="tick")))


@pytest.fixture(scope="module")
def optimized_file_order(input_rrd_path: Path) -> ChunkStore:
    """New engine output with defaults: file-order sweep."""
    return ChunkStore.from_chunks(list(RrdReader(input_rrd_path).store()._optimized_stream()))


def _assert_reader_equality(actual: ChunkStore, baseline: ChunkStore) -> None:
    """Row-for-row data equality through `reader()`, on the fixture's three query shapes."""

    def compare(actual_df: datafusion.DataFrame, baseline_df: datafusion.DataFrame) -> None:
        assert normalized_fields(actual_df.schema()) == normalized_fields(baseline_df.schema())
        assert row_multiset(actual_df) == row_multiset(baseline_df)

    def drop_static(df: datafusion.DataFrame) -> datafusion.DataFrame:
        # On a temporal index, the reader repeats static values once per output batch, so their
        # placement depends on the chunk layout — the very thing the optimizer changes. Static
        # data is compared by the static-only shape below; temporal shapes compare without it.
        return df.drop(*[name for name in df.schema().names if name.startswith("/static:")])

    # Static-only, one-timeline, and two-timeline query shapes.
    compare(actual.reader(index=None), baseline.reader(index=None))
    compare(drop_static(actual.reader(index="tick")), drop_static(baseline.reader(index="tick")))
    compare(drop_static(actual.reader(index="tock")), drop_static(baseline.reader(index="tock")))


def test_data_equality(optimized_timeline_order: ChunkStore, unoptimized: ChunkStore) -> None:
    """The optimizer changes layout, never data."""
    _assert_reader_equality(optimized_timeline_order, unoptimized)


def test_merge_quality(
    optimized_timeline_order: ChunkStore, legacy_optimized: ChunkStore, unoptimized: ChunkStore
) -> None:
    """
    The new engine merges at least as well as the legacy engine — on this fixture.

    By construction per entity: first-fit packs optimally on uniform sizes, and the out-of-order
    entity's time-ordered sweep reaches its byte lower bound. Not a general claim; the general
    case is measured at cutover.
    """
    assert len(optimized_timeline_order) <= len(legacy_optimized)
    assert len(optimized_timeline_order) < len(unoptimized)


def test_layout_snapshot(optimized_timeline_order: ChunkStore, snapshot: SnapshotAssertion) -> None:
    """Snapshot the optimized layout so it changes consciously."""
    assert optimized_timeline_order.summary() == snapshot


def test_laziness(input_rrd_path: Path) -> None:
    """No chunk loads until the stream is consumed — the optimizer plans from the index alone."""
    store: LazyStore = RrdReader(input_rrd_path).store()
    stream = store._optimized_stream(target_timeline="tick")
    assert store._chunks_loaded == 0

    chunks = list(stream)
    assert chunks
    assert store._chunks_loaded > 0


def test_chunk_store_path(unoptimized: ChunkStore) -> None:
    """
    `ChunkStore._optimized_stream` also merges, and preserves data.

    Count parity with the `LazyStore` path is not asserted: the two paths bin on different byte
    metrics and order chunks differently.
    """
    optimized = ChunkStore.from_chunks(list(unoptimized._optimized_stream(target_timeline="tick")))
    assert len(optimized) < len(unoptimized)
    _assert_reader_equality(optimized, unoptimized)


def test_sweep_order_observable(
    optimized_timeline_order: ChunkStore, optimized_file_order: ChunkStore, unoptimized: ChunkStore
) -> None:
    """
    The sweep order is observable in the chunk count.

    The out-of-order entity packs to 6 bins under the time-ordered sweep and 9 under file order —
    and the sweep order changes layout, never data.
    """
    assert len(optimized_timeline_order) < len(optimized_file_order)
    _assert_reader_equality(optimized_file_order, unoptimized)
