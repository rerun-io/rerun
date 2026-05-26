"""Shared helpers used by both the iterable and map-style Rerun datasets."""

from __future__ import annotations

import multiprocessing
import os
import sys
import warnings
from collections import defaultdict
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
from datafusion import col

from rerun._tracing import attach_parent_carrier, current_trace_carrier, tracing_scope, with_tracing
from rerun.catalog import CatalogClient

from ._sample_index import _ns_to_datetime64

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
    index_value: int | np.datetime64
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
        self._view = dataset.filter_contents(_derive_content_filter(self._fields))
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
) -> tuple[list[Target], dict[str, pa.Table]]:
    """Run the server query for `indices` and return `(targets, per-segment tables)`."""
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
    query_indices = _build_query_indices(
        targets,
        fields,
        decoders,
        sample_index=sample_index,
    )

    # Narrow the view so the server can prune at the Lance partition level
    # instead of relying on `using_index_values` alone.
    df = view.filter_segments(list(query_indices.keys())).reader(
        index=index,
        using_index_values=query_indices,
        fill_latest_at=True,
    )

    # `index` and `rerun_segment_id` are preserved because `_decode_iter` and `_split_by_segment` read them.
    select_exprs = [col(index), col("rerun_segment_id")]
    select_exprs.extend(col(field.path).alias(key) for key, field in fields.items())
    arrow_table = df.select(*select_exprs).to_arrow_table()
    seg_tables = _split_by_segment(arrow_table)

    return targets, seg_tables


def _decode_iter(
    *,
    targets: list[Target],
    seg_tables: dict[str, pa.Table],
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
) -> Iterator[dict[str, torch.Tensor | None]]:
    """Yield decoded samples one at a time from a pre-fetched arrow chunk."""
    with tracing_scope("RerunDataset._decode_chunk"):
        for target in targets:
            with tracing_scope("RerunDataset._decode_sample"):
                seg_table = seg_tables.get(target.segment.segment_id)
                if seg_table is None:
                    raise RuntimeError(
                        f"No rows returned for segment {target.segment.segment_id!r} at index {target.index_value!r}"
                    )
                sample: dict[str, torch.Tensor | None] = {}
                index_array = seg_table[index]
                for key, field in fields.items():
                    decoder = decoders[key]
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
                    sample[key] = decoder.decode(raw, target.index_value, target.segment.segment_id)
            yield sample


def _field_index_range(
    idx_val: int | np.datetime64,
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
        lo: Any = _ns_to_datetime64(prior_keyframe) if isinstance(idx_val, np.datetime64) else prior_keyframe
        return lo, idx_val
    return decoder.context_range(idx_val)


def _build_query_indices(
    targets: list[Target],
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    *,
    sample_index: SampleIndex,
) -> dict[str, np.ndarray]:
    """
    Group `targets` by segment, expanded with each field's window and decoder context.

    Returns a `{segment_id: index_values}` dict ready for
    `reader(using_index_values=...)`. Values are `int64` for integer
    indices and `datetime64[ns]` for timestamp timelines.
    """
    is_timestamp = sample_index.is_timestamp
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

    result: dict[str, np.ndarray] = {}
    for segment_id, vals in groups.items():
        arr = np.array(sorted(vals), dtype=np.int64)
        if is_timestamp:
            arr = arr.view("datetime64[ns]")
        result[segment_id] = arr
    return result


@with_tracing("RerunDataset._fetch_prior_keyframes")
def _fetch_prior_keyframes(
    *,
    view: Any,
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    located: list[tuple[SegmentMetadata, int | np.datetime64]],
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
    (ns-since-epoch for timestamp timelines). The caller bisects via
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

    # Anchor columns may not exist in the schema (e.g. pre-optimize data with
    # no user-logged `is_keyframe`); drop those fields so the caller falls back
    # to the decoder heuristic rather than tripping a planner error on a missing
    # column.
    schema_columns = set(view.schema().column_names())
    keyframe_fields = {k: p for k, p in keyframe_fields.items() if p in schema_columns}
    if not keyframe_fields:
        return {}

    is_timestamp = sample_index.is_timestamp

    # Per-segment max target across all anchor-using fields.
    max_per_segment: dict[str, int] = {}
    for seg, idx_val in located:
        sid = seg.segment_id
        iv = int(idx_val)
        max_per_segment[sid] = max(iv, max_per_segment.get(sid, iv))

    unique_paths = list(dict.fromkeys(keyframe_fields.values()))

    def idx_lit(value: int) -> Any:
        return np.datetime64(value, "ns") if is_timestamp else value

    seg_exprs = [
        (col("rerun_segment_id") == seg_id) & (col(index) <= idx_lit(max_t))
        for seg_id, max_t in max_per_segment.items()
    ]
    seg_filter = seg_exprs[0]
    for e in seg_exprs[1:]:
        seg_filter = seg_filter | e

    # `is_keyframe` is `List<Bool>` in Arrow. Datafusion can't coerce that to
    # `Bool`, so `is_not_null()` is a coarse server-side pre-filter. The actual
    # value check happens client-side in the `by_path` loop below.
    # TODO(isaac): Will be able to do check server side with upcoming DF changes.
    path_filter = col(unique_paths[0]).is_not_null()
    for p in unique_paths[1:]:
        path_filter = path_filter | col(p).is_not_null()

    table = (
        view
        .filter_segments(list(max_per_segment.keys()))
        .reader(index=index)
        .filter(seg_filter & path_filter)
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
