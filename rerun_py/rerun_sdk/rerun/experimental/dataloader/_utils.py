"""Shared helpers used by both the iterable and map-style Rerun datasets."""

from __future__ import annotations

import contextvars
import multiprocessing
import os
import sys
import warnings
from collections import defaultdict
from concurrent.futures import Future, ThreadPoolExecutor
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
from datafusion import col

from rerun._tracing import attach_parent_carrier, current_trace_carrier, tracing_scope, with_tracing
from rerun.catalog import CatalogClient

from ._sample_index import _ns_to_datetime64, _ns_to_timedelta64

if TYPE_CHECKING:
    from collections.abc import Iterator

    import torch

    from rerun.experimental._selector import Selector

    from ._config import Field
    from ._decoders import ColumnDecoder
    from ._sample_index import SampleIndex, SegmentMetadata


@dataclass(frozen=True, slots=True)
class Target:
    """One sample to produce."""

    segment: SegmentMetadata
    index_value: int | np.datetime64 | np.timedelta64
    anchors: dict[str, int]


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
        self._view: Any = None
        self._decoders: dict[str, ColumnDecoder] = {}

    @with_tracing("RerunDataset._ensure_initialized")
    def ensure(self) -> tuple[Any, dict[str, ColumnDecoder]]:
        """Return `(view, decoders)`, building them once per worker process."""
        pid = os.getpid()
        if self._initialized and self._init_pid == pid:
            return self._view, self._decoders

        client = CatalogClient(self._catalog_url)
        dataset = client.get_dataset(self._dataset_name)
        self._decoders = {k: f.decode for k, f in self._fields.items()}
        # Leave the dataset unscoped here: each read group narrows contents to its own
        # entities at query time (`_fetch_group`, `_fetch_prior_keyframes`). A shared
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
    *,
    view: Any,
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    sample_index: SampleIndex,
    indices: np.ndarray | list[int],
) -> tuple[list[Target], dict[str, dict[str, pa.Table]]]:
    """
    Run the server queries for `indices` and return `(targets, per-field tables)`.

    Fields are partitioned into read groups so each group queries only its own
    index values: a heavy keyframe-anchored column (video) is fetched over its
    `[keyframe, target]` window alone, not the union with every other field's
    window. The returned mapping is `field_key -> {segment_id -> table}`.
    """
    located = [sample_index.global_to_local(int(idx)) for idx in indices]
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

    groups = _read_groups(fields, decoders)
    group_results = _fetch_groups_parallel(
        groups,
        view=view,
        index=index,
        decoders=decoders,
        sample_index=sample_index,
        targets=targets,
    )

    seg_tables: dict[str, dict[str, pa.Table]] = {}
    for group_fields, group_tables in group_results:
        for key in group_fields:
            seg_tables[key] = group_tables

    return targets, seg_tables


def _fetch_groups_parallel(
    groups: list[tuple[bool, dict[str, Field]]],
    *,
    view: Any,
    index: str,
    decoders: dict[str, ColumnDecoder],
    sample_index: SampleIndex,
    targets: list[Target],
) -> list[tuple[dict[str, Field], dict[str, pa.Table]]]:
    """
    Fetch every read group, overlapping them when there is more than one.

    Each group is an independent server round-trip, so a thread per group lets
    them run concurrently instead of back-to-back: the catalog query releases the
    GIL while it waits on the server. Each thread runs under a copy of the
    caller's context so its `_fetch_group` tracing spans stay nested under
    `_fetch_arrow`.
    """

    def fetch(fill_latest_at: bool, group_fields: dict[str, Field]) -> dict[str, pa.Table]:
        return _fetch_group(
            view=view,
            index=index,
            fields=group_fields,
            decoders=decoders,
            sample_index=sample_index,
            targets=targets,
            fill_latest_at=fill_latest_at,
        )

    if len(groups) == 1:
        fill_latest_at, group_fields = groups[0]
        return [(group_fields, fetch(fill_latest_at, group_fields))]

    with ThreadPoolExecutor(max_workers=len(groups), thread_name_prefix="rerun-fetch-group") as executor:
        futures: list[tuple[dict[str, Field], Future[dict[str, pa.Table]]]] = [
            (group_fields, executor.submit(contextvars.copy_context().run, fetch, fill_latest_at, group_fields))
            for fill_latest_at, group_fields in groups
        ]
        return [(group_fields, future.result()) for group_fields, future in futures]


def _read_groups(
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
) -> list[tuple[bool, dict[str, Field]]]:
    """
    Partition `fields` into read groups, each fetched by one server query.

    Grouped by `(ColumnDecoder.fill_latest_at, is keyframe-anchored)`, since
    `fill_latest_at` is a per-query argument and anchored fields need their own
    `[keyframe, target]` index values rather than the shared window union.
    Returns `(fill_latest_at, group_fields)` pairs.
    """
    groups: dict[tuple[bool, bool], dict[str, Field]] = defaultdict(dict)
    for key, field in fields.items():
        decoder = decoders[key]
        anchored = decoder.prior_keyframe_path(field.path) is not None
        groups[(decoder.fill_latest_at, anchored)][key] = field
    return [(fill_latest_at, group_fields) for (fill_latest_at, _anchored), group_fields in groups.items()]


def _fetch_group(
    *,
    view: Any,
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    sample_index: SampleIndex,
    targets: list[Target],
    fill_latest_at: bool,
) -> dict[str, pa.Table]:
    """Run one server query over the index values one read group needs, split per segment."""
    anchored = any(decoders[key].prior_keyframe_path(field.path) is not None for key, field in fields.items())
    group = f"{'anchored' if anchored else 'windowed'},{'fill' if fill_latest_at else 'exact'}"
    with tracing_scope(f"RerunDataset._fetch_group[{group}]"):
        query_indices = _build_query_indices(targets, fields, decoders, sample_index=sample_index)

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

        # `index` and `rerun_segment_id` are preserved because `_decode_iter` and `_split_by_segment` read them.
        select_exprs = [col(index), col("rerun_segment_id")]
        select_exprs.extend(col(field.path).alias(key) for key, field in fields.items())

        with tracing_scope(f"RerunDataset._fetch_group.to_arrow_table[{group}]"):
            arrow_table = df.select(*select_exprs).to_arrow_table()

        return _split_by_segment(arrow_table)


def _decode_iter(
    *,
    targets: list[Target],
    seg_tables: dict[str, dict[str, pa.Table]],
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
) -> Iterator[dict[str, torch.Tensor | None]]:
    """Yield decoded samples one at a time from the pre-fetched per-field arrow chunks."""
    with tracing_scope("RerunDataset._decode_chunk"):
        for target in targets:
            with tracing_scope("RerunDataset._decode_sample"):
                segment_id = target.segment.segment_id
                sample: dict[str, torch.Tensor | None] = {}
                for key, field in fields.items():
                    decoder = decoders[key]
                    seg_table = seg_tables[key].get(segment_id)
                    if seg_table is None:
                        raise RuntimeError(
                            f"No rows returned for field {key!r} in segment {segment_id!r} at index {target.index_value!r}"
                        )
                    index_array = seg_table[index]
                    lo, hi = _field_index_range(
                        target.index_value, field, decoder, prior_keyframe=target.anchors.get(key)
                    ) or (target.index_value, target.index_value)
                    mask = pc.and_(
                        pc.greater_equal(index_array, lo),
                        pc.less_equal(index_array, hi),
                    )
                    raw = seg_table.filter(mask).column(key)
                    if field.select is not None:
                        raw = _apply_selector(field.select, raw)
                    sample[key] = decoder.decode(raw, target.index_value, segment_id)
            yield sample


def _field_index_range(
    idx_val: int | np.datetime64 | np.timedelta64,
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
        # `lo` must match `idx_val`'s type, or the pyarrow window mask in
        # `_decode_iter` has no kernel (e.g. `greater_equal(duration, int64)`).
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
    view: Any,
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    located: list[tuple[SegmentMetadata, int | np.datetime64 | np.timedelta64]],
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


def _split_by_segment(table: pa.Table) -> dict[str, pa.Table]:
    """Split a combined table into per-segment tables."""
    seg_col = table.column("rerun_segment_id")
    return {segment_id.as_py(): table.filter(pc.equal(seg_col, segment_id)) for segment_id in pc.unique(seg_col)}


def _apply_selector(selector: Selector, raw: pa.ChunkedArray) -> pa.ChunkedArray:
    """Combine `raw` into a single Arrow array, run the selector on it, and re-wrap the output as a `ChunkedArray`."""
    combined = raw.combine_chunks()
    out = selector.execute(combined)
    if out is None:
        return pa.chunked_array([], type=combined.type)
    return pa.chunked_array([out])


def _derive_content_filter(fields: dict[str, Field]) -> list[str]:
    """Build entity content-filter patterns from field paths (`"/camera:EncodedImage:blob"` -> `"/camera/**"`)."""
    return sorted({f"{f.path.split(':')[0]}/**" for f in fields.values()})
