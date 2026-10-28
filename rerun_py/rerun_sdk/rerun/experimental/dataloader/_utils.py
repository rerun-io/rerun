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
from dataclasses import dataclass, field as dataclass_field, replace
from functools import partial
from typing import TYPE_CHECKING, Any, TypeVar

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
from rerun_bindings import CatalogClientInternal

from ._config import TimeAlignedSampling
from ._sample_index import (
    IndexValue,
    _index_value_to_int,
    _ns_to_datetime64,
    _ns_to_timedelta64,
    _validate_timeline_sampling,
    _window_trims_ns,
)
from .decoders import DecodeRequest, FieldBatch

if TYPE_CHECKING:
    from collections.abc import Callable, Generator, Iterable, Iterator, Mapping, Sequence

    from datafusion.expr import Expr

    from rerun.catalog._entry import DatasetEntry

    from ._config import DataSource, Field, FixedRateSampling
    from ._sample_index import SampleIndex, SegmentMetadata
    from .decoders._base import ColumnDecoder, DecodedResult, DecodedSample, DecodedValue

_DecodedT = TypeVar("_DecodedT")


@dataclass(frozen=True, slots=True)
class QueryPlan:
    """A complete description of one exact-index or contiguous-range server query."""

    fields: dict[str, Field]
    fetch_requests: dict[str, list[FieldFetchRequest]]
    query_indices: dict[str, np.ndarray | pa.Array]
    query_ranges: dict[str, list[tuple[IndexValue, IndexValue]]]
    fill_latest_at: bool
    keyframe_columns: dict[str, str] = dataclass_field(default_factory=dict)


@dataclass(frozen=True, slots=True)
class FetchedGroup:
    """One query plan's fields, paired with the Arrow table its server query returned."""

    fields: dict[str, Field]
    fetch_requests: dict[str, list[FieldFetchRequest]]
    table: pa.Table
    keyframe_columns: dict[str, str] = dataclass_field(default_factory=dict)


@dataclass(frozen=True, slots=True)
class FieldFetchRequest:
    """One sample field's timeline-level requirements, resolved before fetching."""

    sample_position: int
    segment_id: str
    index_value: IndexValue
    decode_index_range: tuple[IndexValue, IndexValue]
    output_index_values: tuple[IndexValue, ...]
    fill_latest_at: bool
    requires_contiguous_fetch: bool
    starts_at_keyframe: bool


@dataclass(frozen=True, slots=True)
class Target:
    """One sample to produce."""

    segment: SegmentMetadata
    index_value: IndexValue
    fetch_requests: dict[str, FieldFetchRequest]


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


@dataclass(frozen=True, slots=True)
class IndexedGroup:
    """One fetched group's requests and table indexed for segment-local row lookup."""

    fields: dict[str, Field]
    fetch_requests: dict[str, list[FieldFetchRequest]]
    indexed_table: IndexedTable
    keyframe_columns: dict[str, str] = dataclass_field(default_factory=dict)


@dataclass(frozen=True, slots=True)
class IndexedBlock:
    """One fetched block whose Arrow tables have been indexed by segment."""

    targets: list[Target]
    groups: list[IndexedGroup]


@dataclass(frozen=True, slots=True)
class PreparedField:
    """One field's Arrow batch and fully resolved decoder inputs."""

    batch: FieldBatch
    requests: list[DecodeRequest]
    num_segments: int


@dataclass(frozen=True, slots=True)
class PreparedBlock:
    """One block's fully resolved decoder inputs."""

    num_samples: int
    fields: dict[str, PreparedField]


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


_DATALOADER_CLIENT_NAME = "rerun-py-dataloader"
"""Reported in `x-rerun-client-version`, so the server can tell dataloader traffic apart from other SDK queries."""


class _WorkerConnection:
    """Per-worker catalog connection, view, and decoders, built lazily."""

    def __init__(
        self,
        *,
        catalog_url: str,
        token: str | None,
        dataset_name: str,
        fields: dict[str, Field],
    ) -> None:
        self._catalog_url = catalog_url
        self._token = token
        self._dataset_name = dataset_name
        self._fields = fields
        self._initialized: bool = False
        self._init_pid: int = -1
        self._view: DatasetEntry | None = None
        self._decoders: dict[str, ColumnDecoder[DecodedValue]] = {}

    @classmethod
    def from_source(cls, source: DataSource, fields: dict[str, Field]) -> _WorkerConnection:
        """Build a connection for a [`DataSource`][rerun.experimental.dataloader.DataSource]'s catalog."""
        catalog = source.dataset.catalog
        return cls(
            catalog_url=catalog.url,
            token=catalog._internal.token,
            dataset_name=source.dataset.name,
            fields=fields,
        )

    @with_tracing("RerunDataset._ensure_initialized")
    def ensure(self) -> tuple[DatasetEntry, dict[str, ColumnDecoder[DecodedValue]]]:
        """Return `(view, decoders)`, building them once per worker process."""
        pid = os.getpid()
        if self._initialized and self._init_pid == pid:
            assert self._view is not None  # always set once `_initialized`
            return self._view, self._decoders

        client = CatalogClient._from_internal(
            CatalogClientInternal(self._catalog_url, self._token, client_name=_DATALOADER_CLIENT_NAME)
        )
        dataset = client.get_dataset(self._dataset_name)
        keyframe_fields = {
            key: path for key, field in self._fields.items() if (path := field.prior_keyframe_path) is not None
        }
        if keyframe_fields:
            schema_columns = set(dataset.schema().column_names())
            missing = {key: path for key, path in keyframe_fields.items() if path not in schema_columns}
            if missing:
                details = ", ".join(f"{key!r}: {path!r}" for key, path in sorted(missing.items()))
                raise ValueError(f"Video fields require an is_keyframe column; missing {details}")
        self._decoders = {k: f.decode for k, f in self._fields.items()}
        # Leave the dataset unscoped here: each query plan narrows contents to its own
        # entities at query time (`_fetch_query`, `_fetch_prior_keyframes`).
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


def _locate_samples(
    indices: np.ndarray | list[int],
    *,
    sample_index: SampleIndex,
    num_fields: int,
) -> list[tuple[SegmentMetadata, IndexValue]]:
    """Resolve global sample positions to typed index values within their segments."""
    located = [sample_index.global_to_local(int(idx)) for idx in indices]
    set_current_span_attributes({
        "rerun.dataloader.fetch.num_requested_indices": len(indices),
        "rerun.dataloader.fetch.num_located_targets": len(located),
        "rerun.dataloader.fetch.num_fields": num_fields,
        "rerun.dataloader.fetch.index_values_bytes_estimate": len(indices) * 8,
    })
    return located


def _build_targets(
    located: Sequence[tuple[SegmentMetadata, IndexValue]],
    keyframes: dict[str, dict[str, np.ndarray]],
    *,
    fields: dict[str, Field],
    sample_index: SampleIndex,
    timeline_sampling: TimeAlignedSampling | FixedRateSampling | None = None,
) -> list[Target]:
    """Compute each sample's output and decode ranges."""
    _validate_timeline_sampling(timeline_sampling, sample_index)
    keyframe_interval = (
        timeline_sampling.keyframe_interval if isinstance(timeline_sampling, TimeAlignedSampling) else None
    )
    video_keys = (
        [key for key, field in fields.items() if is_video_field(field)] if keyframe_interval is not None else keyframes
    )
    targets: list[Target] = []
    for seg, idx_val in located:
        earliest_outputs = {
            key: min(_index_value_to_int(value) for value in sample_index.output_index_values(idx_val, fields[key]))
            for key in video_keys
        }
        if keyframe_interval is not None:
            prior_keyframes = {
                key: earliest // keyframe_interval * keyframe_interval
                for key, earliest in earliest_outputs.items()
                if earliest >= 0
            }
        else:
            prior_keyframes = {
                key: prior_keyframe
                for key, by_seg in keyframes.items()
                if (prior_keyframe := _prior_keyframe(by_seg.get(seg.segment_id), earliest_outputs[key])) is not None
            }
        targets.append(
            _build_target(
                sample_position=len(targets),
                segment=seg,
                index_value=idx_val,
                fields=fields,
                sample_index=sample_index,
                prior_keyframes=prior_keyframes,
            )
        )

    return targets


def _pipeline_blocks(
    blocks: list[np.ndarray],
    *,
    fetch: Callable[[np.ndarray], Any],
    process: Callable[[Any], Iterator[DecodedSample]],
) -> Generator[DecodedSample, None, None]:
    """Process blocks while fetching the next block on a background thread."""
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
            with tracing_scope("RerunDataset._wait_for_fetch_block"):
                set_current_span_attributes({"rerun.dataloader.iter.block.index": i})
                fetched = pending.result()
            pending = submit(blocks[i + 1]) if i + 1 < len(blocks) else None
            samples = process(fetched)
            # Only `samples` may keep the block's fetched tables alive while it is drained.
            del fetched
            yield from samples
    finally:
        with tracing_scope("executor.shutdown"):
            executor.shutdown(wait=False)


def _replay(
    samples: Generator[DecodedSample, None, None],
    order: np.ndarray,
) -> Generator[DecodedSample, None, None]:
    """
    Re-emit a fetch-order sample stream in a known pull `order` (a deterministic queue).

    `order[k]` is the fetch position to emit `k`-th. Decode still runs in fetch order;
    this only buffers decoded samples until their turn, so the buffer never exceeds what
    the manifest's buffer held at build time (the order came from that buffer).

    Closes `samples` on exit, so an early teardown reaches the fetch executor's
    shutdown promptly.
    """
    buffer: dict[int, DecodedSample] = {}
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

    def fetch_group(plan: QueryPlan) -> FetchedGroup:
        return FetchedGroup(
            fields=plan.fields,
            fetch_requests=plan.fetch_requests,
            table=_fetch_query(
                view=view,
                index=index,
                plan=plan,
            ),
            keyframe_columns=plan.keyframe_columns,
        )

    return _run_parallel([partial(fetch_group, plan) for plan in plans])


def is_video_field(field: Field) -> bool:
    """Whether a field's decoder needs a keyframe to start decoding."""
    return field.prior_keyframe_path is not None


def _build_query_plans(
    targets: list[Target],
    fields: dict[str, Field],
    *,
    sample_index: SampleIndex,
    timeline_sampling: TimeAlignedSampling | FixedRateSampling | None = None,
    index: str | None = None,
) -> list[QueryPlan]:
    """
    Partition `fields` and fully resolve one server-query plan per partition.

    A plan fetches every field at the union of the partition's indices or
    ranges, so fields may only share a plan when they have compatible reads.
    Fields are split on three properties:

    - `fill_latest_at`: a per-query argument, not a per-column one.
    - contiguous fetch: keyframe-aware decoders fetch every row in their decode
      ranges, while stateless fields fetch only their explicit output indices.
    - `Field.window`: a windowed field fetches its whole window per sample. An
      unwindowed field (e.g. an image) sharing its query would be shipped at
      every index value in that window instead of once per sample.

    Time-aligned sampling disables latest-at filling for every field and expands
    dense video ranges into exact indices, retaining the original decode requests.
    """
    _validate_timeline_sampling(timeline_sampling, sample_index)
    aligned = isinstance(timeline_sampling, TimeAlignedSampling)
    groups: dict[tuple[bool, bool, tuple[int | float, ...] | None], dict[str, Field]] = defaultdict(dict)
    for key, field in fields.items():
        field_requests = [target.fetch_requests[key] for target in targets]
        if not field_requests:
            continue
        fill_latest_at = field_requests[0].fill_latest_at
        if any(request.fill_latest_at != fill_latest_at for request in field_requests):
            raise RuntimeError(f"Inconsistent fill_latest_at policy for field {key!r}")
        contiguous = any(request.requires_contiguous_fetch for request in field_requests)
        query_fill_latest_at = fill_latest_at if not contiguous and not aligned else False
        groups[(query_fill_latest_at, contiguous, field.window)][key] = field
    decode_order = _decode_order(targets)
    plans: list[QueryPlan] = []
    for (fill_latest_at, contiguous, _window), group_fields in groups.items():
        group_fetch_requests = {
            key: [targets[position].fetch_requests[key] for positions in decode_order for position in positions]
            for key in group_fields
        }
        if aligned and not contiguous:
            group_fetch_requests = {
                key: [replace(request, fill_latest_at=False) for request in requests]
                for key, requests in group_fetch_requests.items()
            }
        query_ranges = _build_query_ranges(group_fetch_requests) if contiguous else {}
        query_indices: dict[str, np.ndarray | pa.Array] = {}
        if contiguous and aligned:
            frame_indices = _expand_frame_ranges(query_ranges)
            if frame_indices is not None:
                query_indices.update(frame_indices)
                query_ranges = {}
        elif not contiguous:
            query_indices = _build_query_indices(group_fetch_requests, sample_index=sample_index)
        keyframe_columns: dict[str, str] = {}
        if aligned:
            reserved = {*group_fields, "rerun_segment_id", index}
            for key, field in group_fields.items():
                if is_video_field(field):
                    alias = f"__rerun_keyframe_{len(keyframe_columns)}"
                    while alias in reserved:
                        alias += "_"
                    reserved.add(alias)
                    keyframe_columns[key] = alias
        plans.append(
            QueryPlan(
                fields=group_fields,
                fetch_requests=group_fetch_requests,
                query_indices=query_indices,
                query_ranges=query_ranges,
                fill_latest_at=fill_latest_at if not contiguous else False,
                keyframe_columns=keyframe_columns,
            )
        )
    return plans


def _or_predicates(predicates: list[Expr]) -> Expr:
    """Combine predicates with logarithmic depth for recursive native query planners."""
    if not predicates:
        raise ValueError("At least one predicate is required")
    while len(predicates) > 1:
        predicates = [
            predicates[i] | predicates[i + 1] if i + 1 < len(predicates) else predicates[i]
            for i in range(0, len(predicates), 2)
        ]
    return predicates[0]


def _fetch_query(
    *,
    view: DatasetEntry,
    index: str,
    plan: QueryPlan,
) -> pa.Table:
    """Execute one fully resolved query plan and materialize its Arrow table."""
    fields, query_indices, query_ranges = plan.fields, plan.query_indices, plan.query_ranges
    is_range_query = bool(query_ranges)
    label = f"{'range' if is_range_query else 'indices'},{'fill' if plan.fill_latest_at else 'exact'}"
    with tracing_scope(f"RerunDataset._fetch_query[{label}]"):
        num_query_indices = sum(len(values) for values in query_indices.values())
        num_query_ranges = sum(len(ranges) for ranges in query_ranges.values())
        segment_ids = list(query_ranges if is_range_query else query_indices)
        set_current_span_attributes({
            "rerun.dataloader.group.num_fields": len(fields),
            "rerun.dataloader.group.num_segments": len(segment_ids),
            "rerun.dataloader.group.num_query_indices": num_query_indices,
            "rerun.dataloader.group.num_index_ranges": num_query_ranges,
            "rerun.dataloader.group.fill_latest_at": plan.fill_latest_at,
            "rerun.dataloader.group.range_query": is_range_query,
        })

        # Scope the query to just this group's entities. Otherwise it fetches (then
        # discards at projection) chunks for every other group's entities too: a scalar
        # group would drag in the heavy `VideoStream:sample` chunks of the video group.
        # The server's projection-based entity narrowing is disabled under `fill_latest_at`,
        # so narrow explicitly here. `using_index_values` pins the row set, so restricting
        # entities cannot change the returned rows or their latest-at fills.
        content_paths = [field.path for field in fields.values()]
        content_paths.extend(
            path for key in plan.keyframe_columns if (path := fields[key].prior_keyframe_path) is not None
        )
        scoped = view.filter_contents(_content_filter_for_paths(content_paths)).filter_segments(segment_ids)
        if is_range_query:
            predicates = []
            for segment_id, ranges in query_ranges.items():
                segment = col("rerun_segment_id") == segment_id
                for lo, hi in ranges:
                    predicates.append(segment & (col(index) >= lo) & (col(index) <= hi))
            df = scoped.reader(index=index).filter(_or_predicates(predicates))
        else:
            df = scoped.reader(
                index=index,
                using_index_values=query_indices,
                fill_latest_at=plan.fill_latest_at,
            )

        # `index` and `rerun_segment_id` are preserved because `_find_segment_boundaries` reads them.
        select_exprs = [col(index), col("rerun_segment_id")]
        select_exprs.extend(col(field.path).alias(key) for key, field in fields.items())
        for key, alias in plan.keyframe_columns.items():
            path = fields[key].prior_keyframe_path
            assert path is not None
            select_exprs.append(col(path).alias(alias))

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
        return max(1, sum(1 for field in fields.values() if is_video_field(field)))
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


def _index_fetched_block(fetched: FetchedBlock, index: str) -> IndexedBlock:
    """Index every fetched group's table for segment-local row lookup."""
    return IndexedBlock(
        targets=fetched.targets,
        groups=[
            IndexedGroup(
                fields=group.fields,
                fetch_requests=group.fetch_requests,
                indexed_table=_find_segment_boundaries(group.table, index),
                keyframe_columns=group.keyframe_columns,
            )
            for group in fetched.fetched_groups
        ],
    )


def _decode_order(targets: list[Target]) -> list[list[int]]:
    """
    Target positions in row order: one group per segment, ascending by index value within it.

    Decoders walk a batch forwards — a video decoder walks each GOP front to back
    — so requests have to arrive in row order rather than sampler order. The
    groups' concatenation is that order; they stay separate so query planning
    and row resolution preserve segment boundaries. Shared by every field of a
    block, since they all decode the same targets.
    """
    by_segment: dict[str, list[int]] = {}
    for position, target in enumerate(targets):
        by_segment.setdefault(target.segment.segment_id, []).append(position)
    for positions in by_segment.values():
        positions.sort(key=lambda position: _index_value_to_int(targets[position].index_value))
    return list(by_segment.values())


def _resolve_decode_requests(
    fetch_requests: Sequence[FieldFetchRequest],
    *,
    indexed_table: IndexedTable,
    keyframe_column: pa.ChunkedArray | None = None,
    data_column: pa.Array | None = None,
) -> list[DecodeRequest]:
    """
    Resolve one field's timeline-level fetch requests to physical Arrow rows.

    Each window is normalized to `int64` index values and then to the rows that
    hold them, so decoders never search. Row lookup goes segment by segment
    because index values only ascend — and only compare — inside a segment's span.
    """
    requests: list[DecodeRequest] = []
    by_segment: dict[str, list[FieldFetchRequest]] = {}
    for fetch_request in fetch_requests:
        by_segment.setdefault(fetch_request.segment_id, []).append(fetch_request)

    for segment_id, segment_requests in by_segment.items():
        span = indexed_table.segment_spans.get(segment_id)
        if span is None:
            continue

        span_start, span_stop = span
        rows = indexed_table.index_values[span_start:span_stop]
        contiguous_positions = [
            position for position, request in enumerate(segment_requests) if request.requires_contiguous_fetch
        ]
        decode_spans: dict[int, tuple[int, int]] = {}
        if contiguous_positions:
            lo_values = np.fromiter(
                (
                    _index_value_to_int(segment_requests[position].decode_index_range[0])
                    for position in contiguous_positions
                ),
                dtype=np.int64,
            )
            hi_values = np.fromiter(
                (
                    _index_value_to_int(segment_requests[position].decode_index_range[1])
                    for position in contiguous_positions
                ),
                dtype=np.int64,
            )
            starts = span_start + np.searchsorted(rows, lo_values, side="left")
            stops = span_start + np.searchsorted(rows, hi_values, side="right")
            decode_spans = {
                position: (start, stop)
                for position, start, stop in zip(
                    contiguous_positions,
                    starts.tolist(),
                    stops.tolist(),
                    strict=True,
                )
            }

        for position, fetch_request in enumerate(segment_requests):
            if keyframe_column is not None and fetch_request.starts_at_keyframe:
                start = decode_spans[position][0]
                expected = _index_value_to_int(fetch_request.decode_index_range[0])
                if start >= span_stop or indexed_table.index_values[start] != expected:
                    continue
                marker = keyframe_column[start].as_py()
                if not isinstance(marker, list) or not marker or marker[0] is not True:
                    continue
                if data_column is not None:
                    value = data_column[start]
                    if not value.is_valid or len(value) == 0:
                        continue
            output_values = np.fromiter(
                (_index_value_to_int(value) for value in fetch_request.output_index_values),
                dtype=np.int64,
            )
            output_rows = span_start + np.searchsorted(rows, output_values, side="right") - 1
            minimum_row = decode_spans[position][0] if fetch_request.requires_contiguous_fetch else span_start
            if output_rows.size == 0 or np.any(output_rows < minimum_row):
                continue
            if (
                not fetch_request.fill_latest_at
                and not fetch_request.requires_contiguous_fetch
                and np.any(indexed_table.index_values[output_rows] != output_values)
            ):
                continue
            output_row_indices = tuple(output_rows.tolist())
            decode_row_indices = (
                tuple(range(*decode_spans[position])) if fetch_request.requires_contiguous_fetch else output_row_indices
            )
            requests.append(
                DecodeRequest(
                    sample_position=fetch_request.sample_position,
                    segment_id=segment_id,
                    index_value=fetch_request.index_value,
                    decode_row_indices=decode_row_indices,
                    output_row_indices=output_row_indices,
                    starts_at_keyframe=fetch_request.starts_at_keyframe,
                )
            )
    return requests


def _resolve_decode_requests_in_block(indexed: IndexedBlock) -> PreparedBlock:
    """Resolve every field's timeline-level requests to physical Arrow rows."""
    prepared_fields: dict[str, PreparedField] = {}
    for group in indexed.groups:
        for key, field in group.fields.items():
            fetch_requests = group.fetch_requests[key]
            column = group.indexed_table.table.column(key).combine_chunks()
            prepared_fields[key] = PreparedField(
                batch=FieldBatch(
                    column=column,
                    select=field.select,
                    is_windowed=field.window is not None,
                ),
                requests=_resolve_decode_requests(
                    fetch_requests,
                    indexed_table=group.indexed_table,
                    keyframe_column=(
                        group.indexed_table.table.column(group.keyframe_columns[key])
                        if key in group.keyframe_columns
                        else None
                    ),
                    data_column=column,
                ),
                num_segments=len(group.indexed_table.segment_spans),
            )
    return PreparedBlock(num_samples=len(indexed.targets), fields=prepared_fields)


@with_tracing("RerunDataset._decode_field_batch")
def _decode_field_batch(
    *,
    prepared_field: PreparedField,
    num_samples: int,
    key: str,
    decoder: ColumnDecoder[_DecodedT],
) -> list[_DecodedT | None]:
    """
    Decode one field for every target of a fetch block; `result[i]` aligns with `targets[i]`.

    The whole block goes to the decoder in a single `decode` call, so a
    stateless decoder gathers every sample at once; results are scattered back
    into target order.
    """
    set_current_span_attributes({
        "rerun.dataloader.decode.field": key,
        "rerun.dataloader.decode.num_requests": len(prepared_field.requests),
        "rerun.dataloader.decode.num_segments": prepared_field.num_segments,
    })

    results = decoder.decode(prepared_field.batch, prepared_field.requests)
    if len(results) != len(prepared_field.requests):
        raise RuntimeError(
            f"{type(decoder).__name__}.decode returned {len(results)} results "
            f"for {len(prepared_field.requests)} requests (field {key!r})"
        )

    out: list[_DecodedT | None] = [None] * num_samples
    for request, result in zip(prepared_field.requests, results, strict=True):
        out[request.sample_position] = result
    return out


def _decode_iter(
    *,
    prepared: PreparedBlock,
    decoders: dict[str, ColumnDecoder[DecodedValue]],
    executor: ThreadPoolExecutor | None = None,
) -> Iterator[DecodedSample]:
    """
    Decode every field of a block now, then yield its samples one at a time.

    The returned iterator holds only decoded values, not `prepared`, so the
    block's fetched Arrow data can be freed before its samples are consumed.
    """
    with tracing_scope("RerunDataset._decode_block"):
        set_current_span_attributes({"rerun.dataloader.decode.block_size": prepared.num_samples})
        if not prepared.num_samples:
            return iter(())

        decode_field: dict[str, Callable[[], list[DecodedResult]]] = {}
        for key, prepared_field in prepared.fields.items():
            decode_field[key] = partial(
                _decode_field_batch,
                prepared_field=prepared_field,
                num_samples=prepared.num_samples,
                key=key,
                decoder=decoders[key],
            )

        if executor is None:
            per_field = {key: decode() for key, decode in decode_field.items()}
        else:
            # Copy the caller's contextvars so each field's spans nest under this
            # block's span instead of appearing as roots.
            futures: dict[str, Future[list[DecodedResult]]] = {  # ty: ignore[invalid-assignment]
                key: executor.submit(contextvars.copy_context().run, decode) for key, decode in decode_field.items()
            }
            per_field = {key: future.result() for key, future in futures.items()}

    return _iter_samples(per_field, prepared.num_samples)


def _iter_samples(per_field: dict[str, list[DecodedResult]], num_samples: int) -> Iterator[DecodedSample]:
    """Yield one sample per position from per-field decoded columns."""
    for i in range(num_samples):
        yield {key: values[i] for key, values in per_field.items()}


def _resolve_decode_index_range(
    idx_val: IndexValue,
    field: Field,
    *,
    output_index_values: tuple[IndexValue, ...],
    prior_keyframe: int | None = None,
) -> tuple[IndexValue, IndexValue] | None:
    """
    Inclusive `(lo, hi)` range of index values needed for one field at `idx_val`, or `None` if only `idx_val` is needed.

    The output bounds define the requested values; a prior keyframe may extend
    the decode start farther back.
    """
    output_lo = min(output_index_values, key=_index_value_to_int)
    output_hi = max(output_index_values, key=_index_value_to_int)
    if prior_keyframe is not None:
        return _index_value_like(prior_keyframe, idx_val), output_hi
    if field.window is not None:
        return output_lo, output_hi
    return None


def _index_value_like(value: int, example: IndexValue) -> IndexValue:
    """Represent an integer timeline value using `example`'s concrete scalar type."""
    if isinstance(example, np.datetime64):
        return _ns_to_datetime64(value)
    if isinstance(example, np.timedelta64):
        return _ns_to_timedelta64(value)
    return value


def _build_target(
    *,
    sample_position: int,
    segment: SegmentMetadata,
    index_value: IndexValue,
    fields: dict[str, Field],
    sample_index: SampleIndex,
    prior_keyframes: dict[str, int] | None = None,
) -> Target:
    """Resolve every field's timeline-level fetch requirements for one sample."""
    prior_keyframes = prior_keyframes or {}
    fetch_requests: dict[str, FieldFetchRequest] = {}
    for key, field in fields.items():
        prior_keyframe = prior_keyframes.get(key)
        output_index_values = sample_index.output_index_values(index_value, field)
        decode_index_range = _resolve_decode_index_range(
            index_value,
            field,
            output_index_values=output_index_values,
            prior_keyframe=prior_keyframe,
        ) or (
            index_value,
            index_value,
        )
        fetch_requests[key] = FieldFetchRequest(
            sample_position=sample_position,
            segment_id=segment.segment_id,
            index_value=index_value,
            decode_index_range=decode_index_range,
            output_index_values=output_index_values,
            fill_latest_at=field.fill_latest_at,
            requires_contiguous_fetch=prior_keyframe is not None,
            starts_at_keyframe=prior_keyframe is not None,
        )
    return Target(segment=segment, index_value=index_value, fetch_requests=fetch_requests)


def _build_query_indices(
    fetch_requests: dict[str, list[FieldFetchRequest]],
    *,
    sample_index: SampleIndex,
) -> dict[str, np.ndarray | pa.Array]:
    """
    Group each field's exact requested output indices by segment.

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

    for field_requests in fetch_requests.values():
        for fetch_request in field_requests:
            segment_values = groups[fetch_request.segment_id]
            segment_values.update(_index_value_to_int(value) for value in fetch_request.output_index_values)

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


_MAX_FRAME_QUERY_INDICES = 65_536
"""Bound each expanded query to 512 KiB of int64 indices before wire encoding."""

_INITIAL_KEYFRAME_LOOKBACK_SAMPLES = 16
"""Keep the first read local to random targets; doubling discovers longer GOPs without a spacing assumption."""


def _expand_frame_ranges(
    query_ranges: Mapping[str, Sequence[tuple[IndexValue, IndexValue]]],
) -> dict[str, np.ndarray] | None:
    """Return exact integer indices, or `None` when the query would be too large."""
    total = 0
    for ranges in query_ranges.values():
        for lo, hi in ranges:
            lower, upper = int(lo), int(hi)
            if not -(1 << 63) <= lower <= upper < (1 << 63):
                raise ValueError("Dense frame ranges must have ordered bounds within signed int64")
            total += upper - lower + 1
            if total > _MAX_FRAME_QUERY_INDICES:
                return None

    return {
        segment_id: np.concatenate([
            np.arange(int(hi) - int(lo) + 1, dtype=np.int64) + np.int64(lo) for lo, hi in ranges
        ])
        for segment_id, ranges in query_ranges.items()
    }


def _build_query_ranges(
    fetch_requests: dict[str, list[FieldFetchRequest]],
) -> dict[str, list[tuple[IndexValue, IndexValue]]]:
    """Merge inclusive decode ranges independently within each segment."""
    ranges_by_segment: dict[str, list[tuple[int, int]]] = defaultdict(list)
    examples: dict[str, IndexValue] = {}
    for requests in fetch_requests.values():
        for request in requests:
            lo, hi = request.decode_index_range
            ranges_by_segment[request.segment_id].append((_index_value_to_int(lo), _index_value_to_int(hi)))
            examples.setdefault(request.segment_id, request.index_value)

    result: dict[str, list[tuple[IndexValue, IndexValue]]] = {}
    for segment_id, ranges in ranges_by_segment.items():
        merged: list[list[int]] = []
        for lo, hi in sorted(ranges):
            if merged and lo <= merged[-1][1] + 1:
                merged[-1][1] = max(merged[-1][1], hi)
            else:
                merged.append([lo, hi])

        example = examples[segment_id]
        result[segment_id] = [(_index_value_like(lo, example), _index_value_like(hi, example)) for lo, hi in merged]

    return result


def _merge_integer_ranges(ranges: Iterable[tuple[int, int]]) -> list[tuple[int, int]]:
    """Merge overlapping or adjacent inclusive ranges without overflowing int64."""
    merged: list[tuple[int, int]] = []
    for lo, hi in sorted(ranges):
        if merged and lo <= merged[-1][1] + 1:
            merged[-1] = (merged[-1][0], max(merged[-1][1], hi))
        else:
            merged.append((lo, hi))
    return merged


def _uncovered_integer_ranges(ranges: list[tuple[int, int]], covered: list[tuple[int, int]]) -> list[tuple[int, int]]:
    """Subtract previously queried ranges from merged, ordered inclusive ranges."""
    result: list[tuple[int, int]] = []
    for lo, hi in ranges:
        for covered_lo, covered_hi in covered:
            if covered_hi < lo:
                continue
            if covered_lo > hi:
                break
            if lo < covered_lo:
                result.append((lo, covered_lo - 1))
            lo = max(lo, covered_hi + 1)
            if lo > hi:
                break
        if lo <= hi:
            result.append((lo, hi))
    return result


@with_tracing("RerunDataset._fetch_keyframe_ranges")
def _fetch_keyframe_ranges(
    *,
    view: DatasetEntry,
    index: str,
    paths: list[str],
    ranges: dict[str, list[tuple[int, int]]],
    sample_index: SampleIndex,
) -> dict[str, dict[str, np.ndarray]]:
    """Read true markers inside bounded ranges, retaining their original timeline values."""
    scoped = view.filter_contents(_content_filter_for_paths(paths)).filter_segments(list(ranges))
    # Integer indices can be enumerated exactly. Time-based sample grids cannot:
    # an actual keyframe may lie between two requested sampling timestamps.
    exact = _expand_frame_ranges(ranges) if sample_index.ns_dtype is None else None
    if exact is not None:
        df = scoped.reader(index=index, using_index_values=exact, fill_latest_at=False)
    else:
        example = (
            _ns_to_datetime64(0)
            if sample_index.is_timestamp
            else _ns_to_timedelta64(0)
            if sample_index.is_duration
            else 0
        )
        predicates = []
        for segment_id, segment_ranges in ranges.items():
            segment = col("rerun_segment_id") == segment_id
            for lo, hi in segment_ranges:
                predicates.append(
                    segment
                    & (col(index) >= _index_value_like(lo, example))
                    & (col(index) <= _index_value_like(hi, example))
                )
        df = scoped.reader(index=index, fill_latest_at=False).filter(_or_predicates(predicates))

    path_filter = col(paths[0]).is_not_null()
    for path in paths[1:]:
        path_filter = path_filter | col(path).is_not_null()
    with tracing_scope("RerunDataset._fetch_keyframe_ranges.to_arrow_table"):
        table = (
            df
            .filter(path_filter)
            .select(col(index), col("rerun_segment_id"), *[col(p) for p in paths])
            .to_arrow_table()
        )

    by_path: dict[str, dict[str, np.ndarray]] = {}
    for path in paths:
        # list_element raises for empty lists; null and empty marker rows carry
        # no keyframe evidence and must be removed before extracting the flag.
        nonempty = pc.fill_null(pc.greater(pc.list_value_length(table.column(path)), 0), False)
        present = table.filter(nonempty)
        marked = present.filter(pc.list_element(present.column(path), 0))
        segments = marked.column("rerun_segment_id").to_pylist()
        values = marked.column(index).to_numpy(zero_copy_only=False).astype(np.int64)
        by_segment: dict[str, list[int]] = defaultdict(list)
        for segment_id, value in zip(segments, values, strict=True):
            by_segment[segment_id].append(int(value))
        by_path[path] = {segment_id: np.unique(values) for segment_id, values in by_segment.items()}
    return by_path


@with_tracing("RerunDataset._discover_prior_keyframes")
def _discover_prior_keyframes(
    *,
    view: DatasetEntry,
    index: str,
    fields: dict[str, Field],
    located: Sequence[tuple[SegmentMetadata, IndexValue]],
    sample_index: SampleIndex,
) -> dict[str, dict[str, np.ndarray]]:
    """
    Discover the nearest prior keyframe for each live sample with bounded backward reads.

    Searches expand only while unresolved, down to the untrimmed segment minimum.
    Each target is resolved only after its entire interval back to a true marker
    has been read, so sparse markers and unrelated target windows cannot hide a
    nearer keyframe. Returned arrays need only cover these targets; manifest
    generation uses `_fetch_prior_keyframes` to obtain the complete history.
    """
    keyframe_fields = {key: path for key, field in fields.items() if (path := field.prior_keyframe_path) is not None}
    if not keyframe_fields or not located:
        return {}
    paths = list(dict.fromkeys(keyframe_fields.values()))
    trim_start = (
        _window_trims_ns(fields)[0]
        if sample_index.ns_dtype is not None
        else -min([0, *(int(min(field.window)) for field in fields.values() if field.window is not None)])
    )
    minimum = -(1 << 63) + int(sample_index.ns_dtype is not None)
    maximum = (1 << 63) - 1
    floors: dict[str, int] = {}
    pending: dict[tuple[str, str, int], int] = {}
    for segment, anchor in located:
        floor = segment.index_min if segment.index_min is not None else segment.index_start - trim_start
        floors[segment.segment_id] = max(minimum, floor)
        for key, path in keyframe_fields.items():
            target = min(_index_value_to_int(value) for value in sample_index.output_index_values(anchor, fields[key]))
            if target >= floors[segment.segment_id]:
                pending[(path, segment.segment_id, target)] = min(target, maximum) + 1

    found: dict[str, dict[str, np.ndarray]] = {path: {} for path in paths}
    covered: dict[str, list[tuple[int, int]]] = {}
    width = _INITIAL_KEYFRAME_LOOKBACK_SAMPLES * (sample_index.ns_per_sample or 1)
    rounds = 0
    while pending:
        requested: dict[str, list[tuple[int, int]]] = defaultdict(list)
        for (_path, segment_id, target), previous_lo in pending.items():
            lo = max(floors[segment_id], min(target, maximum) - width + 1)
            if lo < previous_lo:
                requested[segment_id].append((lo, previous_lo - 1))
        ranges = {
            segment_id: remaining
            for segment_id, spans in requested.items()
            if (remaining := _uncovered_integer_ranges(_merge_integer_ranges(spans), covered.get(segment_id, [])))
        }
        if ranges:
            # Active paths only shrink: every remaining path was projected by
            # every earlier query, so their searched-range coverage is shared.
            pending_paths = {path for path, _segment_id, _target in pending}
            active_paths = [path for path in paths if path in pending_paths]
            fetched = _fetch_keyframe_ranges(
                view=view, index=index, paths=active_paths, ranges=ranges, sample_index=sample_index
            )
            rounds += 1
            for path, by_segment in fetched.items():
                for segment_id, values in by_segment.items():
                    previous = found[path].get(segment_id)
                    found[path][segment_id] = values if previous is None else np.union1d(previous, values)
            for segment_id, spans in ranges.items():
                covered[segment_id] = _merge_integer_ranges([*covered.get(segment_id, []), *spans])

        unresolved: dict[tuple[str, str, int], int] = {}
        for (path, segment_id, target), _previous_lo in pending.items():
            lo = max(floors[segment_id], min(target, maximum) - width + 1)
            prior = _prior_keyframe(found[path].get(segment_id), target)
            if (prior is None or prior < lo) and lo > floors[segment_id]:
                unresolved[(path, segment_id, target)] = lo
        pending = unresolved
        width *= 2

    set_current_span_attributes({"rerun.dataloader.keyframes.discovery_rounds": rounds})
    return {key: found[path] for key, path in keyframe_fields.items()}


@with_tracing("RerunDataset._fetch_prior_keyframes")
def _fetch_prior_keyframes(
    *,
    view: DatasetEntry,
    index: str,
    fields: dict[str, Field],
    located: Sequence[tuple[SegmentMetadata, IndexValue]],
    sample_index: SampleIndex,
) -> dict[str, dict[str, np.ndarray]]:
    """
    Per-field sorted keyframe index values, grouped by segment.

    Skips fields without a `prior_keyframe_path`. Fields that declare one must
    have that keyframe column in the live schema. Returns `{}` when no field
    needs keyframe metadata, so non-video datasets pay no query overhead.

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
        path = field.prior_keyframe_path
        if path is not None:
            keyframe_fields[key] = path
    if not keyframe_fields:
        return {}

    if not located:
        return {}

    # Per-segment max requested output across all keyframe-aware fields.
    max_per_segment: dict[str, int] = {}
    for seg, idx_val in located:
        sid = seg.segment_id
        requested_max = max(
            _index_value_to_int(value)
            for key in keyframe_fields
            for value in sample_index.output_index_values(idx_val, fields[key])
        )
        max_per_segment[sid] = max(requested_max, max_per_segment.get(sid, requested_max))

    unique_paths = list(dict.fromkeys(keyframe_fields.values()))

    def idx_lit(value: int) -> Any:
        # The literal must match the index column type, or DataFusion fails to
        # coerce the comparison (`Duration(ns) <= Int64`).
        if sample_index.is_timestamp:
            return np.datetime64(value, "ns")
        if sample_index.is_duration:
            return np.timedelta64(value, "ns")
        return value

    # Manifest generation needs every preceding keyframe, not only the nearest
    # one for each representative target. A global upper bound includes that
    # complete history; per-sample bisects select the appropriate prior marker.
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
    # Scope to just the keyframe entities (the `is_keyframe` siblings live on the same
    # entities as the video samples), so this query never touches unrelated entities.
    keyframe_contents = _content_filter_for_paths(unique_paths)

    with tracing_scope("RerunDataset._fetch_prior_keyframes.to_arrow_table"):
        table = (
            view
            .filter_contents(keyframe_contents)
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
    return _content_filter_for_paths(field.path for field in fields.values())


def _content_filter_for_paths(paths: Iterable[str]) -> list[str]:
    """Build entity content-filter patterns without splitting escaped colons in entity paths."""
    return sorted({f"{path.rsplit(':', 2)[0]}/**" for path in paths})
