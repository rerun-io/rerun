"""Shared helpers used by both the iterable and map-style Rerun datasets."""

from __future__ import annotations

import contextvars
import multiprocessing
import os
import sys
import warnings
from collections import defaultdict
from concurrent.futures import Future, ThreadPoolExecutor
from contextlib import contextmanager
from dataclasses import dataclass
from functools import partial
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
from datafusion import col

from rerun._tracing import (
    attach_parent_carrier,
    current_trace_carrier,
    set_current_span_attributes,
    tracing_scope,
    with_tracing,
)
from rerun.catalog import CatalogClient

from ._sample_index import IndexValue, _ns_to_datetime64, _ns_to_timedelta64
from .decoders import DecodeRequest, FieldBatch

if TYPE_CHECKING:
    from collections.abc import Callable, Generator, Iterator, Sequence

    import torch

    from rerun.catalog._entry import DatasetEntry

    from ._config import DataSource, Field
    from ._sample_index import SampleIndex, SegmentMetadata
    from .decoders import ColumnDecoder


@dataclass(frozen=True, slots=True)
class QueryPlan:
    """A complete description of one server query for one fetch block."""

    fields: dict[str, Field]
    query_indices: dict[str, np.ndarray | pa.Array]
    fill_latest_at: bool
    anchored: bool


@dataclass(frozen=True, slots=True)
class FetchedGroup:
    """One query plan's fields, paired with the Arrow table its server query returned."""

    fields: dict[str, Field]
    table: pa.Table


@dataclass(frozen=True, slots=True)
class Target:
    """One sample to produce."""

    segment: SegmentMetadata
    index_value: IndexValue
    anchors: dict[str, int]


@dataclass(frozen=True, slots=True)
class FetchedBlock:
    """One fetch block's logical targets and materialized query results."""

    targets: list[Target]
    fetched_groups: list[FetchedGroup]


@dataclass(frozen=True, slots=True)
class IndexedTable:
    """A fetched table indexed by its numeric timeline values and contiguous segment row spans."""

    table: pa.Table
    index_values: np.ndarray
    segment_spans: dict[str, tuple[int, int]]


def _warn_if_fork_unsafe(stacklevel: int) -> None:
    """
    Warn when DataLoader workers will be started with `fork`.

    Rerun's `rerun_bindings` extension uses a process-global tokio runtime.
    `fork` only carries the calling thread into the child, so the runtime's
    worker threads vanish and the first catalog call from a DataLoader
    worker deadlocks. Only `spawn` (and `forkserver`) are currently safe.
    """
    method = multiprocessing.get_start_method(allow_none=True)
    will_be_fork = method == "fork" or (method is None and sys.platform.startswith("linux"))
    if not will_be_fork:
        return
    warnings.warn(
        "The default multiprocessing start method is 'fork'. The Rerun "
        "dataloader needs 'spawn' or 'forkserver' for DataLoader workers "
        "(num_workers > 0). Forked workers will deadlock on their first "
        "catalog call. Pass "
        "`multiprocessing_context=multiprocessing.get_context('spawn')` to "
        "your DataLoader, or call "
        "`torch.multiprocessing.set_start_method('spawn')` before creating "
        "workers. You can ignore this warning if you use num_workers=0.",
        RuntimeWarning,
        stacklevel=stacklevel,
    )


class _WorkerConnection:
    """Per-worker catalog connection, view, and decoders, built lazily."""

    def __init__(
        self,
        *,
        catalog_url: str,
        dataset_name: str,
        fields: dict[str, Field],
    ) -> None:
        self._catalog_url = catalog_url
        self._dataset_name = dataset_name
        self._fields = fields
        self._initialized: bool = False
        self._init_pid: int = -1
        self._view: DatasetEntry | None = None
        self._decoders: dict[str, ColumnDecoder] = {}

    @classmethod
    def from_source(cls, source: DataSource, fields: dict[str, Field]) -> _WorkerConnection:
        """Build a connection for a [`DataSource`][rerun.experimental.dataloader.DataSource]'s catalog."""
        return cls(catalog_url=source.dataset.catalog.url, dataset_name=source.dataset.name, fields=fields)

    @with_tracing("RerunDataset._ensure_initialized")
    def ensure(self) -> tuple[DatasetEntry, dict[str, ColumnDecoder]]:
        """Return `(view, decoders)`, building them once per worker process."""
        pid = os.getpid()
        if self._initialized and self._init_pid == pid:
            assert self._view is not None  # always set once `_initialized`
            return self._view, self._decoders

        client = CatalogClient(self._catalog_url)
        dataset = client.get_dataset(self._dataset_name)
        self._decoders = {k: f.decode for k, f in self._fields.items()}
        # Leave the dataset unscoped here: each query plan narrows contents to its own
        # entities at query time (`_fetch_query`, `_fetch_prior_keyframes`). A shared
        # union filter here would defeat that, since `filter_contents` only ever widens,
        # so a group could never exclude the other groups' (heavy video) entities.
        self._view = dataset
        self._initialized = True
        self._init_pid = pid
        return self._view, self._decoders

    def __getstate__(self) -> dict[str, Any]:
        """Drop the cached view so the worker rebuilds its own connection via `ensure()`."""
        state = self.__dict__.copy()
        state["_view"] = None
        state["_initialized"] = False
        # Capture the parent's OTel context so worker spans are linked to it.
        state["_parent_trace_carrier"] = current_trace_carrier()
        return state

    def __setstate__(self, state: dict[str, Any]) -> None:
        self.__dict__.update(state)
        attach_parent_carrier(state.get("_parent_trace_carrier"))


@with_tracing("RerunDataset._fetch_arrow")
def _fetch_arrow(
    indices: np.ndarray | list[int],
    *,
    view: DatasetEntry,
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    sample_index: SampleIndex,
) -> FetchedBlock:
    """
    Run the server queries for `indices` and return their targets and fetched tables as one block.

    Fields are partitioned into query plans so each query reads only its own
    index values: a heavy keyframe-anchored column (video) is fetched over its
    `[keyframe, target]` window alone, not the union with every other field's
    window. Each plan is materialized as a `FetchedGroup`, which every field
    represented by that table then decodes from.
    """
    located = [sample_index.global_to_local(int(idx)) for idx in indices]
    set_current_span_attributes({
        "rerun.dataloader.fetch.num_requested_indices": len(indices),
        "rerun.dataloader.fetch.num_located_targets": len(located),
        "rerun.dataloader.fetch.num_fields": len(fields),
        "rerun.dataloader.fetch.num_segments": len({seg.segment_id for seg, _ in located}),
        "rerun.dataloader.fetch.index_values_bytes_estimate": len(indices) * 8,
    })
    keyframes = _fetch_prior_keyframes(
        view=view,
        index=index,
        fields=fields,
        decoders=decoders,
        located=located,
        sample_index=sample_index,
    )
    targets: list[Target] = []
    for seg, idx_val in located:
        iv = int(idx_val)
        anchors: dict[str, int] = {}
        for key, by_seg in keyframes.items():
            kf = _prior_keyframe(by_seg.get(seg.segment_id), iv)
            if kf is not None:
                anchors[key] = kf
        targets.append(Target(segment=seg, index_value=idx_val, anchors=anchors))

    return _fetch_targets(targets, view=view, index=index, fields=fields, decoders=decoders, sample_index=sample_index)


def _fetch_targets(
    targets: list[Target],
    *,
    view: DatasetEntry,
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    sample_index: SampleIndex,
) -> FetchedBlock:
    """Plan and run every server query needed for already-resolved `targets`."""
    plans = _build_query_plans(targets, fields, decoders, sample_index=sample_index)
    return FetchedBlock(
        targets=targets,
        fetched_groups=_fetch_queries_parallel(
            plans,
            view=view,
            index=index,
        ),
    )


def _interleave_fetch_and_decode(
    blocks: list[np.ndarray],
    *,
    fetch: Callable[[np.ndarray], Any],
    decode: Callable[[Any], Iterator[dict[str, torch.Tensor | None]]],
) -> Generator[dict[str, torch.Tensor | None], None, None]:
    """Yield decoded samples, fetching block N+1 on a background thread while block N is decoded."""
    if not blocks:
        return

    executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix="rerun-fetch")

    def submit(block: np.ndarray) -> Future[Any]:
        # Copy the caller's contextvars so the fetch's span nests under the current OTel
        # context instead of appearing as a root trace.
        ctx = contextvars.copy_context()
        return executor.submit(lambda: ctx.run(fetch, block))

    try:
        pending: Future[Any] | None = submit(blocks[0])
        for i in range(len(blocks)):
            assert pending is not None
            fetched = pending.result()
            pending = submit(blocks[i + 1]) if i + 1 < len(blocks) else None
            yield from decode(fetched)
    finally:
        with tracing_scope("executor.shutdown"):
            executor.shutdown(wait=False)


def _replay(
    samples: Generator[dict[str, torch.Tensor | None], None, None],
    order: np.ndarray,
) -> Generator[dict[str, torch.Tensor | None], None, None]:
    """
    Re-emit a fetch-order sample stream in a known pull `order` (a deterministic queue).

    `order[k]` is the fetch position to emit `k`-th. Decode still runs in fetch order;
    this only buffers decoded samples until their turn, so the buffer never exceeds what
    the manifest's buffer held at build time (the order came from that buffer).

    Closes `samples` on exit, so an early teardown reaches the fetch executor's
    shutdown promptly.
    """
    buffer: dict[int, dict[str, torch.Tensor | None]] = {}
    fetched = enumerate(samples)
    try:
        for fetch_idx in order:
            fetch_idx = int(fetch_idx)
            while fetch_idx not in buffer:
                i, sample = next(fetched)
                buffer[i] = sample
            yield buffer.pop(fetch_idx)
    finally:
        samples.close()


def _run_parallel(tasks: list[Callable[[], Any]], *, max_workers: int | None = None) -> list[Any]:
    """
    Run independent, GIL-releasing tasks concurrently, returning results in task order.

    Each task runs under a copy of the caller's context so its tracing spans stay
    nested under the current span, and runs inline when there is a single task.
    Intended for catalog queries, which release the GIL while waiting on the server.
    `max_workers` caps the pool so a large task list runs in bounded waves rather
    than one thread per task (defaults to one thread per task).
    """
    if not tasks:
        return []
    if len(tasks) == 1:
        return [tasks[0]()]
    workers = min(len(tasks), max_workers) if max_workers else len(tasks)
    with ThreadPoolExecutor(max_workers=workers, thread_name_prefix="rerun-parallel") as executor:
        futures = [executor.submit(contextvars.copy_context().run, task) for task in tasks]
        return [future.result() for future in futures]


def _fetch_queries_parallel(
    plans: list[QueryPlan],
    *,
    view: DatasetEntry,
    index: str,
) -> list[FetchedGroup]:
    """Execute every query plan, overlapping the independent server round-trips."""

    def fetch(plan: QueryPlan) -> FetchedGroup:
        return FetchedGroup(
            fields=plan.fields,
            table=_fetch_query(
                view=view,
                index=index,
                plan=plan,
            ),
        )

    return _run_parallel([partial(fetch, plan) for plan in plans])


def is_video_field(field: Field, decoder: ColumnDecoder) -> bool:
    """Whether a field is keyframe-anchored (compressed video), i.e. its decode window starts at a prior keyframe."""
    return decoder.prior_keyframe_path(field.path) is not None


def _build_query_plans(
    targets: list[Target],
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    *,
    sample_index: SampleIndex,
) -> list[QueryPlan]:
    """
    Partition `fields` and fully resolve one server-query plan per partition.

    A plan fetches every field at the union of the partition's index
    values, so fields may only share a plan when they need the same rows.
    Fields are split on three properties:

    - `fill_latest_at`: a per-query argument, not a per-column one.
    - keyframe-anchored: anchored fields (video) fetch a `[keyframe, target]`
      range per sample; no other field wants those rows.
    - `Field.window`: a windowed field fetches its whole window per sample. An
      unwindowed field (e.g. an image) sharing its query would be shipped at
      every index value in that window instead of once per sample.
    """
    groups: dict[tuple[bool, bool, tuple[int, int] | None], dict[str, Field]] = defaultdict(dict)
    for key, field in fields.items():
        decoder = decoders[key]
        # A field with an explicit window is never anchored, even when its
        # decoder wants keyframes (`_fetch_prior_keyframes` skips it).
        anchored = field.window is None and decoder.prior_keyframe_path(field.path) is not None
        groups[(decoder.fill_latest_at, anchored, field.window)][key] = field
    plans: list[QueryPlan] = []
    for (fill_latest_at, anchored, _window), group_fields in groups.items():
        plans.append(
            QueryPlan(
                fields=group_fields,
                query_indices=_build_query_indices(
                    targets,
                    group_fields,
                    decoders,
                    sample_index=sample_index,
                ),
                fill_latest_at=fill_latest_at,
                anchored=anchored,
            )
        )
    return plans


def _fetch_query(
    *,
    view: DatasetEntry,
    index: str,
    plan: QueryPlan,
) -> pa.Table:
    """Execute one fully resolved query plan and materialize its Arrow table."""
    fields, query_indices, fill_latest_at = plan.fields, plan.query_indices, plan.fill_latest_at
    label = f"{'anchored' if plan.anchored else 'windowed'},{'fill' if fill_latest_at else 'exact'}"
    with tracing_scope(f"RerunDataset._fetch_query[{label}]"):
        num_query_indices = sum(len(values) for values in query_indices.values())
        set_current_span_attributes({
            "rerun.dataloader.group.num_fields": len(fields),
            "rerun.dataloader.group.num_segments": len(query_indices),
            "rerun.dataloader.group.num_query_indices": num_query_indices,
            "rerun.dataloader.group.fill_latest_at": fill_latest_at,
            "rerun.dataloader.group.anchored": plan.anchored,
        })

        # Scope the query to just this group's entities. Otherwise it fetches (then
        # discards at projection) chunks for every other group's entities too: a scalar
        # group would drag in the heavy `VideoStream:sample` chunks of the video group.
        # The server's projection-based entity narrowing is disabled under `fill_latest_at`,
        # so narrow explicitly here. `using_index_values` pins the row set, so restricting
        # entities cannot change the returned rows or their latest-at fills.
        df = (
            view
            .filter_contents(_derive_content_filter(fields))
            .filter_segments(list(query_indices.keys()))
            .reader(
                index=index,
                using_index_values=query_indices,
                fill_latest_at=fill_latest_at,
            )
        )

        # `index` and `rerun_segment_id` are preserved because `_find_segment_boundaries` reads them.
        select_exprs = [col(index), col("rerun_segment_id")]
        select_exprs.extend(col(field.path).alias(key) for key, field in fields.items())

        with tracing_scope(f"RerunDataset._fetch_query.to_arrow_table[{label}]"):
            arrow_table = df.select(*select_exprs).to_arrow_table()
            set_current_span_attributes({
                "rerun.dataloader.group.arrow_rows": arrow_table.num_rows,
                "rerun.dataloader.group.arrow_columns": arrow_table.num_columns,
                "rerun.dataloader.group.arrow_nbytes": arrow_table.nbytes,
            })

        return arrow_table


def _resolve_decode_threads(decode_threads: int | None, fields: dict[str, Field]) -> int:
    """The per-worker decode fan-out, defaulting to one thread per video field."""
    if decode_threads is None:
        return max(1, sum(1 for field in fields.values() if is_video_field(field, field.decode)))
    if decode_threads < 1:
        raise ValueError(f"decode_threads must be at least 1, got {decode_threads}")
    return decode_threads


@contextmanager
def _decode_pool(decode_threads: int, num_fields: int) -> Generator[ThreadPoolExecutor | None, None, None]:
    """
    A decode pool for one iteration pass, or `None` when fields are decoded sequentially.

    Sized to `min(decode_threads, num_fields)`: a sample's fields are joined before
    it is yielded, so threads beyond the field count would never be scheduled.
    """
    workers = min(decode_threads, num_fields)
    if workers <= 1:
        yield None
        return
    executor = ThreadPoolExecutor(max_workers=workers, thread_name_prefix="rerun-decode")
    try:
        yield executor
    finally:
        executor.shutdown(wait=False)


def _find_segment_boundaries(table: pa.Table, index: str) -> IndexedTable:
    """
    Put one fetched group's rows in per-segment order, and say where each segment sits.

    Returns an [`IndexedTable`][] whose `index_values` is the index column as
    `int64` (ns for temporal indices) and whose `segment_spans` maps a segment to
    its `[start, stop)` row range. Within a span the index values are ascending,
    so a lookup there is a `searchsorted`; across spans they restart, which is
    why every lookup has to name a segment.

    The reader already returns rows grouped by segment and ordered by index, so
    the common path only verifies that — no copy. A table that violates it is
    sorted once here, so a reader change degrades to a one-time sort rather than
    silently slicing the wrong rows.
    """
    values = table.column(index).combine_chunks().to_numpy(zero_copy_only=False)
    values = values.view(np.int64) if values.dtype.kind in "mM" else values.astype(np.int64, copy=False)
    if values.size == 0:
        return IndexedTable(table=table, index_values=values, segment_spans={})

    # Compare dictionary codes rather than segment id strings, so both the
    # boundary scan and the ascending check are single vectorized passes.
    encoded = table.column("rerun_segment_id").combine_chunks().dictionary_encode()
    codes = encoded.indices.to_numpy(zero_copy_only=False)
    names = encoded.dictionary.to_pylist()

    same_segment = codes[1:] == codes[:-1]
    boundaries = np.flatnonzero(~same_segment) + 1
    # One boundary per segment transition means no segment is split in two. A
    # descent is only a problem inside a segment; across a boundary it is normal.
    if boundaries.size + 1 != len(names) or np.any((np.diff(values) < 0) & same_segment):
        order = np.lexsort((values, codes))
        table = table.take(pa.array(order))
        values, codes = values[order], codes[order]
        boundaries = np.flatnonzero(codes[1:] != codes[:-1]) + 1

    starts = [0, *boundaries.tolist()]
    stops = [*boundaries.tolist(), values.size]
    spans = {names[int(codes[start])]: (start, stop) for start, stop in zip(starts, stops, strict=True)}
    return IndexedTable(table=table, index_values=values, segment_spans=spans)


def _decode_order(targets: list[Target]) -> list[list[int]]:
    """
    Target positions in row order: one group per segment, ascending by index value within it.

    Decoders walk a batch forwards — a video decoder walks each GOP front to back
    — so requests have to arrive in row order rather than sampler order. The
    groups' concatenation is that order; they stay separate so `_resolve_decode_requests`
    can resolve rows one segment at a time. Shared by every field of a block,
    since they all decode the same targets.
    """
    by_segment: dict[str, list[int]] = {}
    for position, target in enumerate(targets):
        by_segment.setdefault(target.segment.segment_id, []).append(position)
    for positions in by_segment.values():
        positions.sort(key=lambda position: int(targets[position].index_value))
    return list(by_segment.values())


def _resolve_decode_requests(
    targets: list[Target],
    order: list[list[int]],
    *,
    indexed_table: IndexedTable,
    key: str,
    field: Field,
    decoder: ColumnDecoder,
) -> list[DecodeRequest]:
    """
    Resolve one field's decode window for every target, as requests in `order`.

    Each window is normalized to `int64` index values and then to the rows that
    hold them, so decoders never search. Row lookup goes segment by segment
    because index values only ascend — and only compare — inside a segment's span.
    """
    requests: list[DecodeRequest] = []
    for positions in order:
        segment_id = targets[positions[0]].segment.segment_id
        span = indexed_table.segment_spans.get(segment_id)
        if span is None:
            raise RuntimeError(
                f"No rows returned for field {key!r} in segment {segment_id!r} "
                f"at index {targets[positions[0]].index_value!r}"
            )

        anchored: list[bool] = []
        lo_values = np.empty(len(positions), dtype=np.int64)
        hi_values = np.empty(len(positions), dtype=np.int64)
        for i, position in enumerate(positions):
            target = targets[position]
            anchor = target.anchors.get(key)
            lo, hi = _field_index_range(target.index_value, field, decoder, prior_keyframe=anchor) or (
                target.index_value,
                target.index_value,
            )
            anchored.append(anchor is not None)
            lo_values[i] = int(lo)
            hi_values[i] = int(hi)

        span_start, span_stop = span
        rows = indexed_table.index_values[span_start:span_stop]
        starts = span_start + np.searchsorted(rows, lo_values, side="left")
        stops = span_start + np.searchsorted(rows, hi_values, side="right")
        # `tolist()` once rather than unboxing numpy scalars per request.
        requests.extend(
            DecodeRequest(
                segment_id=segment_id,
                index_value=targets[position].index_value,
                rows=range(start, stop),
                starts_at_keyframe=starts_at_keyframe,
            )
            for position, starts_at_keyframe, start, stop in zip(
                positions, anchored, starts.tolist(), stops.tolist(), strict=True
            )
        )
    return requests


@with_tracing("RerunDataset._decode_field_batch")
def _decode_field_batch(
    *,
    targets: list[Target],
    order: list[list[int]],
    indexed_table: IndexedTable,
    key: str,
    field: Field,
    decoder: ColumnDecoder,
) -> list[torch.Tensor | None]:
    """
    Decode one field for every target of a fetch block; `result[i]` aligns with `targets[i]`.

    The whole block goes to the decoder in a single `decode` call, so a
    stateless decoder gathers every sample at once; results are scattered back
    into target order.
    """
    requests = _resolve_decode_requests(
        targets, order, indexed_table=indexed_table, key=key, field=field, decoder=decoder
    )
    set_current_span_attributes({
        "rerun.dataloader.decode.field": key,
        "rerun.dataloader.decode.num_requests": len(requests),
        "rerun.dataloader.decode.num_segments": len(indexed_table.segment_spans),
    })

    batch = FieldBatch(column=indexed_table.table.column(key).combine_chunks(), select=field.select)
    results = decoder.decode(batch, requests)
    if len(results) != len(requests):
        raise RuntimeError(
            f"{type(decoder).__name__}.decode returned {len(results)} results "
            f"for {len(requests)} requests (field {key!r})"
        )

    out: list[torch.Tensor | None] = [None] * len(targets)
    positions = [position for group in order for position in group]
    for position, result in zip(positions, results, strict=True):
        out[position] = result
    return out


def _decode_iter(
    *,
    fetched: FetchedBlock,
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    executor: ThreadPoolExecutor | None = None,
) -> Iterator[dict[str, torch.Tensor | None]]:
    """Yield decoded samples one at a time from a block's materialized query tables."""
    with tracing_scope("RerunDataset._decode_block"):
        targets = fetched.targets
        set_current_span_attributes({"rerun.dataloader.iter.block.num_targets": len(targets)})
        if not targets:
            return

        order = _decode_order(targets)
        decode_field: dict[str, Callable[[], list[torch.Tensor | None]]] = {}
        for fetched_group in fetched.fetched_groups:
            # Once per table, not per field: its fields came from one query, so
            # they share row order and segment spans.
            indexed_table = _find_segment_boundaries(fetched_group.table, index)
            for key, field in fetched_group.fields.items():
                decode_field[key] = partial(
                    _decode_field_batch,
                    targets=targets,
                    order=order,
                    indexed_table=indexed_table,
                    key=key,
                    field=field,
                    decoder=decoders[key],
                )

        if executor is None:
            per_field = {key: decode() for key, decode in decode_field.items()}
        else:
            # Copy the caller's contextvars so each field's spans nest under this
            # block's span instead of appearing as roots.
            futures: dict[str, Future[list[torch.Tensor | None]]] = {
                key: executor.submit(contextvars.copy_context().run, decode) for key, decode in decode_field.items()
            }
            per_field = {key: future.result() for key, future in futures.items()}
        for i in range(len(targets)):
            yield {key: per_field[key][i] for key in fields}


def _field_index_range(
    idx_val: IndexValue,
    field: Field,
    decoder: ColumnDecoder,
    *,
    prior_keyframe: int | None = None,
) -> tuple[Any, Any] | None:
    """
    Inclusive `(lo, hi)` range of index values needed for one field at `idx_val`, or `None` if only `idx_val` is needed.

    Precedence: `Field.window` > `prior_keyframe` > `ColumnDecoder.context_range`.
    """
    if field.window is not None:
        return idx_val + field.window[0], idx_val + field.window[1]
    if prior_keyframe is not None:
        # `lo` must match `idx_val`'s type, so callers comparing or arithmetic-
        # combining the pair (e.g. `_build_query_indices`) see one dtype.
        if isinstance(idx_val, np.datetime64):
            lo: Any = _ns_to_datetime64(prior_keyframe)
        elif isinstance(idx_val, np.timedelta64):
            lo = _ns_to_timedelta64(prior_keyframe)
        else:
            lo = prior_keyframe
        return lo, idx_val
    return decoder.context_range(idx_val)


def _build_query_indices(
    targets: list[Target],
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    *,
    sample_index: SampleIndex,
) -> dict[str, np.ndarray | pa.Array]:
    """
    Group `targets` by segment, expanded with each field's window and decoder context.

    Returns a `{segment_id: index_values}` dict ready for
    `reader(using_index_values=...)`. Values are an `int64` ndarray for
    integer indices, a `pa.timestamp("ns")` array for timestamp
    timelines, and a `pa.duration("ns")` array for duration timelines.
    The Rust `IndexValuesLike` binding only accepts `datetime64`
    ndarrays among the temporal numpy dtypes, so temporal values cross
    the binding as pyarrow arrays — matching the convention used by
    `TimeColumn` in `_send_columns.py`.
    """
    ns_dtype = sample_index.ns_dtype
    groups: dict[str, set[int]] = defaultdict(set)

    for target in targets:
        segment_id = target.segment.segment_id

        groups[segment_id].add(int(target.index_value))

        for key, field in fields.items():
            anchor = target.anchors.get(key)
            rng = _field_index_range(target.index_value, field, decoders[key], prior_keyframe=anchor)
            if rng is None:
                continue
            lo, hi = rng
            for val in sample_index.indices_in_range(int(lo), int(hi)):
                groups[segment_id].add(int(val))
            # The keyframe's exact index value is unlikely to land on a fixed-rate
            # grid; ensure the main fetch returns its row regardless.
            if anchor is not None:
                groups[segment_id].add(anchor)

    result: dict[str, np.ndarray | pa.Array] = {}
    for segment_id, vals in groups.items():
        arr = np.array(sorted(vals), dtype=np.int64)
        if ns_dtype == "datetime64[ns]":
            result[segment_id] = pa.array(arr, type=pa.timestamp("ns"))
        elif ns_dtype == "timedelta64[ns]":
            result[segment_id] = pa.array(arr, type=pa.duration("ns"))
        else:
            result[segment_id] = arr
    return result


@with_tracing("RerunDataset._fetch_prior_keyframes")
def _fetch_prior_keyframes(
    *,
    view: DatasetEntry,
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    located: Sequence[tuple[SegmentMetadata, IndexValue]],
    sample_index: SampleIndex,
) -> dict[str, dict[str, np.ndarray]]:
    """
    Per-field sorted keyframe index values, grouped by segment.

    Skips fields with `Field.window` set, decoders whose `prior_keyframe_path`
    returns `None`, and anchor paths absent from the live schema. Returns `{}`
    when no field needs an anchor, so non-video datasets pay no query overhead.

    Queries `is_keyframe` rows at or before each segment's max target.
    Works whether `is_keyframe` is logged sparsely (only `true` on keyframes)
    or densely (`true`/`false` on every row). The result maps
    `field_key -> {segment_id: sorted_int64_keyframes}`; values are `int`
    (ns-since-epoch for timestamp timelines, ns count for duration timelines).
    The caller bisects via
    [`_prior_keyframe`][rerun.experimental.dataloader._utils._prior_keyframe].
    """
    keyframe_fields: dict[str, str] = {}
    for key, field in fields.items():
        if field.window is not None:
            continue
        path = decoders[key].prior_keyframe_path(field.path)
        if path is not None:
            keyframe_fields[key] = path
    if not keyframe_fields or not located:
        return {}

    # Anchor columns may not exist in the schema (e.g. pre-optimize data with no user-logged `is_keyframe`)
    # drop those fields so the caller falls back to the decoder heuristic
    schema_columns = set(view.schema().column_names())
    keyframe_fields = {k: p for k, p in keyframe_fields.items() if p in schema_columns}
    if not keyframe_fields:
        return {}

    # Per-segment max target across all anchor-using fields.
    max_per_segment: dict[str, int] = {}
    for seg, idx_val in located:
        sid = seg.segment_id
        iv = int(idx_val)
        max_per_segment[sid] = max(iv, max_per_segment.get(sid, iv))

    unique_paths = list(dict.fromkeys(keyframe_fields.values()))

    def idx_lit(value: int) -> Any:
        # The literal must match the index column type, or DataFusion fails to
        # coerce the comparison (`Duration(ns) <= Int64`).
        if sample_index.is_timestamp:
            return np.datetime64(value, "ns")
        if sample_index.is_duration:
            return np.timedelta64(value, "ns")
        return value

    # Filter to keyframes at or before the largest target across all segments, in a
    # single predicate. A per-segment OR (`(seg==A & idx<=tA) | (seg==B & idx<=tB) | …`)
    # is expanded server-side into one `QueryDataset` request per segment, each planned
    # serially, so the cost scales with segment count. Using the global max instead
    # collapses that to a single request; segments whose own target is lower over-fetch
    # a few extra keyframe rows (sparse, tiny), and the client-side `_prior_keyframe`
    # bisect still selects the correct keyframe per segment per target. Segments are
    # already restricted by `filter_segments` below.
    global_max = max(max_per_segment.values())
    index_filter = col(index) <= idx_lit(global_max)

    # `is_keyframe` is `List<Bool>` in Arrow. Datafusion can't coerce that to
    # `Bool`, so `is_not_null()` is a coarse server-side pre-filter. The actual
    # value check happens client-side in the `by_path` loop below.
    # TODO(isaac): Will be able to do check server side with upcoming DF changes.
    path_filter = col(unique_paths[0]).is_not_null()
    for p in unique_paths[1:]:
        path_filter = path_filter | col(p).is_not_null()

    # Selecting only the `is_keyframe` columns (a strict subset of the entity's
    # components) under the default `fill_latest_at=False` lets the server push this
    # projection into each query's `fuzzy_descriptors` and skip chunks for the heavy
    # `VideoStream:sample` sibling. Keep the select narrow and do not pass
    # `fill_latest_at=True`, or the push-down (gated on `SparseFillStrategy::None`)
    # falls back to fetching every component on the entity.
    # Scope to just the anchor entities (the `is_keyframe` siblings live on the same
    # entities as the video samples), so this query never touches unrelated entities.
    anchor_contents = sorted({f"{p.split(':')[0]}/**" for p in unique_paths})

    with tracing_scope("RerunDataset._fetch_prior_keyframes.to_arrow_table"):
        table = (
            view
            .filter_contents(anchor_contents)
            .filter_segments(list(max_per_segment.keys()))
            .reader(index=index)
            .filter(index_filter & path_filter)
            .select(col(index), col("rerun_segment_id"), *[col(p) for p in unique_paths])
            .to_arrow_table()
        )

    # Per-path: sorted int64 arrays of keyframe index values, grouped by segment.
    # `int(scalar)` on a `datetime64[ns]` element returns its nanoseconds-since-epoch
    # representation, so this works uniformly for int64 and timestamp timelines.
    by_path: dict[str, dict[str, np.ndarray]] = {}
    for path in unique_paths:
        mask = pc.list_element(table.column(path), 0)
        sub = table.filter(mask)
        sub_segs = sub.column("rerun_segment_id").to_pylist()
        sub_idx = sub.column(index).to_numpy(zero_copy_only=False)
        by_seg: dict[str, list[int]] = defaultdict(list)
        for s, v in zip(sub_segs, sub_idx, strict=True):
            by_seg[s].append(int(v))
        by_path[path] = {s: np.sort(np.array(vs, dtype=np.int64)) for s, vs in by_seg.items()}

    return {key: by_path[path] for key, path in keyframe_fields.items()}


def _prior_keyframe(sorted_kfs: np.ndarray | None, target: int) -> int | None:
    """Largest value in *sorted_kfs* that is `<=` *target*, or `None` if none exists."""
    if sorted_kfs is None or len(sorted_kfs) == 0:
        return None
    pos = int(np.searchsorted(sorted_kfs, target, side="right")) - 1
    return None if pos < 0 else int(sorted_kfs[pos])


def _derive_content_filter(fields: dict[str, Field]) -> list[str]:
    """Build entity content-filter patterns from field paths (`"/camera:EncodedImage:blob"` -> `"/camera/**"`)."""
    return sorted({f"{f.path.split(':')[0]}/**" for f in fields.values()})
