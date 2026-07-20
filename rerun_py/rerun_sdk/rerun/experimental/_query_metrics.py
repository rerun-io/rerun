"""
Programmatic capture of DataFusion query metrics from Python.

`re_datafusion` records plan-time and per-partition fetch metrics on every
dataset query (`query_chunks`, `filters_pushed_down`, `fetch_grpc_bytes`, …).
On the Rust side these surface in `EXPLAIN ANALYZE`; from Python they
*should* surface via `df.explain(analyze=True)`, but a bug in
`datafusion-python` / `datafusion_ffi` currently strips the metrics when the
plan crosses the FFI capsule. A fix is in flight upstream.

In the meantime, this module exposes [`query_metrics`][] — a context manager
that captures the same metrics directly from the Rust side, bypassing
DataFusion's FFI:

```python
from rerun.experimental import query_metrics

with query_metrics() as m:
    df = dataset.reader(index="time_1").limit(100)
    df.collect()
    print(m.last_query())
```

Each query that runs inside the `with` block produces one
[`QueryMetrics`][] record (built when the last per-partition stream
finishes). Mid-scope reads via `m.queries` or `m.last_query()` are
non-destructive; on `__exit__` any remaining snapshots are drained into the
collector and the scope is unbound.

`query_metrics()` is part of `rerun.experimental` — once the upstream
DataFusion FFI fix lands, `df.explain(analyze=True)` starts working and this
API may evolve (or be removed) without going through the standard
deprecation cycle.
"""

from __future__ import annotations

import contextlib
import logging
from contextvars import ContextVar
from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import datetime
    from collections.abc import Iterator

logger = logging.getLogger("rerun")


# Stack of currently-active `_MetricsCollectorHandle`s, scoped to the current
# `contextvars.Context`. The Rust side reads this in
# `rerun_py/src/catalog/dataset_view.rs::reader()` to bind metrics capture to
# the queries built inside an active scope — and nothing else.
#
# Stored as a tuple (immutable, cheap to copy on push) so each
# `_active_collectors.set(…)` produces a fresh value that the ContextVar
# token can reset cleanly. The name "object" rather than the concrete
# `_MetricsCollectorHandle` keeps this module importable when the catalog
# bindings aren't available in the local build.
_active_collectors: ContextVar[tuple[object, ...]] = ContextVar("rerun_query_metrics_collectors", default=())  # NOLINT


@dataclass(frozen=True)
class QueryMetrics:
    """
    One query's metrics, captured at the moment its last per-partition stream finished.

    Mirrors the Rust-side `re_datafusion::QuerySnapshot`. The same numbers are
    produced via three transports: this dataclass (Python), DataFusion's
    `EXPLAIN ANALYZE`, and the PostHog analytics OTLP span. Field naming
    differs across the three:

    - Timing fields here are `datetime.timedelta` (`total_duration`,
      `time_to_first_chunk`, …). `EXPLAIN ANALYZE` uses DataFusion `Time`
      metrics, which print their own units. The OTLP analytics attributes
      keep an explicit `_us` suffix and carry integer microseconds
      (`total_duration_us`, `time_to_first_chunk_us`, …) because OTLP
      attribute values are scalar (`i64` / `f64` / `bool` / `string`) and
      can't carry a duration natively.
    - `query_chunks_per_segment_mean` is a `float` and does not appear in
      `EXPLAIN ANALYZE`, since DataFusion `Count` metrics are integer-only.
      The corresponding `_min` / `_max` integer fields are surfaced in all
      three transports.

    `fetch_direct_max_attempt` is the true maximum attempt number across all
    partitions.
    """

    # Plan-time
    dataset_id: str
    query_chunks: int
    query_segments: int
    query_layers: int
    query_columns: int
    query_entities: int
    query_bytes: int
    query_chunks_per_segment_min: int
    query_chunks_per_segment_max: int
    query_chunks_per_segment_mean: float
    query_type: str
    primary_index_name: str | None
    time_to_first_chunk_info: datetime.timedelta | None
    filters_pushed_down: int
    filters_applied_client_side: int
    entity_path_narrowing_applied: bool

    # Execution-time
    total_duration: datetime.timedelta
    time_to_first_chunk: datetime.timedelta | None
    error_kind: str | None
    direct_terminal_reason: str | None

    # Fetch counters (summed across partitions)
    fetch_grpc_requests: int
    fetch_grpc_bytes: int
    fetch_direct_requests: int
    fetch_direct_bytes: int
    fetch_direct_retries: int
    fetch_direct_requests_retried: int
    fetch_direct_retry_sleep: datetime.timedelta
    fetch_direct_max_attempt: int
    fetch_direct_original_ranges: int
    fetch_direct_merged_ranges: int

    # Scheduling and admission counters
    planned_fetch_batches: int
    planned_segment_waves: int
    segment_admission_limit: int
    max_segments_per_fetch_batch: int
    max_segments_per_wave: int
    peak_active_segments: int
    pipeline_budget_bytes: int
    pipeline_peak_decoded_bytes: int
    pipeline_byte_waits: int
    segment_admission_waits: int
    pipeline_stall_breaker_activations: int

    @property
    def fetch_requests(self) -> int:
        """Total fetch requests across both gRPC and direct transports."""
        return self.fetch_grpc_requests + self.fetch_direct_requests

    @property
    def fetch_bytes(self) -> int:
        """Total bytes fetched across both gRPC and direct transports."""
        return self.fetch_grpc_bytes + self.fetch_direct_bytes


def _from_rust(m: object) -> QueryMetrics:
    """Build a `QueryMetrics` from a Rust-side `_QueryMetrics` PyO3 instance."""
    return QueryMetrics(
        dataset_id=m.dataset_id,  # type: ignore[attr-defined]
        query_chunks=m.query_chunks,  # type: ignore[attr-defined]
        query_segments=m.query_segments,  # type: ignore[attr-defined]
        query_layers=m.query_layers,  # type: ignore[attr-defined]
        query_columns=m.query_columns,  # type: ignore[attr-defined]
        query_entities=m.query_entities,  # type: ignore[attr-defined]
        query_bytes=m.query_bytes,  # type: ignore[attr-defined]
        query_chunks_per_segment_min=m.query_chunks_per_segment_min,  # type: ignore[attr-defined]
        query_chunks_per_segment_max=m.query_chunks_per_segment_max,  # type: ignore[attr-defined]
        query_chunks_per_segment_mean=m.query_chunks_per_segment_mean,  # type: ignore[attr-defined]
        query_type=m.query_type,  # type: ignore[attr-defined]
        primary_index_name=m.primary_index_name,  # type: ignore[attr-defined]
        time_to_first_chunk_info=m.time_to_first_chunk_info,  # type: ignore[attr-defined]
        filters_pushed_down=m.filters_pushed_down,  # type: ignore[attr-defined]
        filters_applied_client_side=m.filters_applied_client_side,  # type: ignore[attr-defined]
        entity_path_narrowing_applied=m.entity_path_narrowing_applied,  # type: ignore[attr-defined]
        total_duration=m.total_duration,  # type: ignore[attr-defined]
        time_to_first_chunk=m.time_to_first_chunk,  # type: ignore[attr-defined]
        error_kind=m.error_kind,  # type: ignore[attr-defined]
        direct_terminal_reason=m.direct_terminal_reason,  # type: ignore[attr-defined]
        fetch_grpc_requests=m.fetch_grpc_requests,  # type: ignore[attr-defined]
        fetch_grpc_bytes=m.fetch_grpc_bytes,  # type: ignore[attr-defined]
        fetch_direct_requests=m.fetch_direct_requests,  # type: ignore[attr-defined]
        fetch_direct_bytes=m.fetch_direct_bytes,  # type: ignore[attr-defined]
        fetch_direct_retries=m.fetch_direct_retries,  # type: ignore[attr-defined]
        fetch_direct_requests_retried=m.fetch_direct_requests_retried,  # type: ignore[attr-defined]
        fetch_direct_retry_sleep=m.fetch_direct_retry_sleep,  # type: ignore[attr-defined]
        fetch_direct_max_attempt=m.fetch_direct_max_attempt,  # type: ignore[attr-defined]
        fetch_direct_original_ranges=m.fetch_direct_original_ranges,  # type: ignore[attr-defined]
        fetch_direct_merged_ranges=m.fetch_direct_merged_ranges,  # type: ignore[attr-defined]
        planned_fetch_batches=m.planned_fetch_batches,  # type: ignore[attr-defined]
        planned_segment_waves=m.planned_segment_waves,  # type: ignore[attr-defined]
        segment_admission_limit=m.segment_admission_limit,  # type: ignore[attr-defined]
        max_segments_per_fetch_batch=m.max_segments_per_fetch_batch,  # type: ignore[attr-defined]
        max_segments_per_wave=m.max_segments_per_wave,  # type: ignore[attr-defined]
        peak_active_segments=m.peak_active_segments,  # type: ignore[attr-defined]
        pipeline_budget_bytes=m.pipeline_budget_bytes,  # type: ignore[attr-defined]
        pipeline_peak_decoded_bytes=m.pipeline_peak_decoded_bytes,  # type: ignore[attr-defined]
        pipeline_byte_waits=m.pipeline_byte_waits,  # type: ignore[attr-defined]
        segment_admission_waits=m.segment_admission_waits,  # type: ignore[attr-defined]
        pipeline_stall_breaker_activations=m.pipeline_stall_breaker_activations,  # type: ignore[attr-defined]
    )


class MetricsCollector:
    """
    Accumulator yielded by [`query_metrics`][rerun.experimental.query_metrics] on `__enter__`.

    Use `last_query()` / `queries` to read snapshots accumulated so far; both
    are non-destructive. On context-manager exit any remaining snapshots are
    drained into this collector and the scope is unbound from the
    `ContextVar`, so the collector is still readable after the scope ends.
    """

    def __init__(self, handle: object | None) -> None:
        # `handle` is the Rust `_MetricsCollectorHandle`. `None` means
        # allocation failed and this collector is inert — every operation
        # returns the current `_finalized` snapshot list (empty by default).
        self._handle = handle
        self._finalized: list[QueryMetrics] = []

    @property
    def queries(self) -> list[QueryMetrics]:
        """Non-destructive snapshot of all queries captured so far."""
        if self._handle is None:
            return list(self._finalized)
        live = [_from_rust(m) for m in self._handle.snapshot()]  # type: ignore[attr-defined]
        # After scope exit the Rust handle still works, but `_finalize` has
        # already moved the buffer into `_finalized`. Combine both so the
        # collector is fully readable post-`with` block.
        return self._finalized + live

    def last_query(self) -> QueryMetrics | None:
        """Most recently captured query, or `None` if none yet."""
        qs = self.queries
        return qs[-1] if qs else None

    def clear(self) -> None:
        """Drop all captured snapshots from both the Rust buffer and this collector."""
        self._finalized.clear()
        if self._handle is not None:
            self._handle.drain()  # type: ignore[attr-defined]

    def _finalize(self) -> None:
        """Move any remaining Rust-side snapshots into `_finalized`. Called on `__exit__`."""
        if self._handle is None:
            return
        drained = [_from_rust(m) for m in self._handle.drain()]  # type: ignore[attr-defined]
        self._finalized.extend(drained)


@contextlib.contextmanager
def query_metrics() -> Iterator[MetricsCollector]:
    """
    Capture DataFusion query metrics for every query that runs inside the `with` block.

    Yields a [`MetricsCollector`][rerun.experimental.MetricsCollector]; read `.last_query()` or
    `.queries` mid-scope or after the scope exits.

    The scope is bound to the current `contextvars.Context`: every
    `re_datafusion` query built from `dataset.reader(…)` while this scope
    is open contributes a `QueryMetrics` record. Nested `query_metrics()`
    scopes each see queries built inside them. Queries built in another
    thread or `asyncio` task that did **not** inherit this context (e.g. a
    raw `threading.Thread` rather than one started via
    `contextvars.copy_context()`) are *not* captured.

    The collectors are bound to a query at `reader()` time, so a `df` built
    inside the `with` block whose `.collect()` runs after `__exit__` still
    flows to the collector; a `df` built outside but executed inside does
    not.

    Examples
    --------
    ```python
    import rerun as rr
    from rerun.experimental import query_metrics

    client = rr.catalog.CatalogClient("rerun://…")
    dataset = client.get_dataset(name="…")

    with query_metrics() as m:
        df = dataset.reader(index="time_1").limit(100)
        df.collect()
        print(m.last_query())
    ```

    """
    try:
        from rerun_bindings import _new_metrics_collector
    except ImportError:
        # The PyO3 bindings haven't been built with the catalog feature; the
        # bridge isn't available. Yield an inert collector so user code still
        # runs.
        logger.warning(
            "rerun.experimental.query_metrics() is a no-op: the catalog "
            "bindings are not available in this build of rerun.",
        )
        yield MetricsCollector(handle=None)
        return

    try:
        handle = _new_metrics_collector()
    except Exception:  # pragma: no cover — defensive; PyO3 allocation shouldn't fail
        logger.exception("Failed to allocate query_metrics collector; yielding inert collector.")
        yield MetricsCollector(handle=None)
        return

    token = _active_collectors.set((*_active_collectors.get(), handle))
    collector = MetricsCollector(handle=handle)
    try:
        yield collector
    finally:
        try:
            collector._finalize()
        finally:
            _active_collectors.reset(token)
