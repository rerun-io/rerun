"""Shared helpers used by both the iterable and map-style Rerun datasets."""

from __future__ import annotations

import multiprocessing
import os
import sys
import warnings
from collections import defaultdict
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
from datafusion import col

from rerun._tracing import attach_parent_carrier, current_trace_carrier, tracing_scope, with_tracing
from rerun.catalog import CatalogClient

if TYPE_CHECKING:
    from collections.abc import Iterator

    import torch

    from rerun.experimental._selector import Selector

    from ._config import Field
    from ._decoders import ColumnDecoder
    from ._sample_index import SampleIndex, SegmentMetadata

#: (segment_metadata, index_value) pair identifying one sample to produce.
Target = tuple["SegmentMetadata", "int | np.datetime64"]


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
    targets: list[Target] = [sample_index.global_to_local(int(idx)) for idx in indices]
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
        for seg_meta, idx_val in targets:
            with tracing_scope("RerunDataset._decode_sample"):
                seg_table = seg_tables.get(seg_meta.segment_id)
                if seg_table is None:
                    raise RuntimeError(f"No rows returned for segment {seg_meta.segment_id!r} at index {idx_val!r}")
                sample: dict[str, torch.Tensor | None] = {}
                index_array = seg_table[index]
                for key, field in fields.items():
                    decoder = decoders[key]
                    lo, hi = _field_index_range(idx_val, field, decoder) or (idx_val, idx_val)
                    mask = pc.and_(
                        pc.greater_equal(index_array, lo),
                        pc.less_equal(index_array, hi),
                    )
                    raw = seg_table.filter(mask).column(key)
                    if field.select is not None:
                        raw = _apply_selector(field.select, raw)
                    sample[key] = decoder.decode(raw, idx_val, seg_meta.segment_id)
            yield sample


def _field_index_range(
    idx_val: int | np.datetime64,
    field: Field,
    decoder: ColumnDecoder,
) -> tuple[Any, Any] | None:
    """
    Inclusive `(lo, hi)` range of index values needed for one field at `idx_val`, or `None` if only `idx_val` is needed.

    `Field.window` takes precedence over `ColumnDecoder.context_range`.
    """
    if field.window is not None:
        return idx_val + field.window[0], idx_val + field.window[1]
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

    for seg_meta, idx_val in targets:
        segment_id = seg_meta.segment_id

        groups[segment_id].add(int(idx_val))

        for key, field in fields.items():
            rng = _field_index_range(idx_val, field, decoders[key])
            if rng is None:
                continue
            lo, hi = rng
            for val in sample_index.indices_in_range(int(lo), int(hi)):
                groups[segment_id].add(int(val))

    result: dict[str, np.ndarray] = {}
    for segment_id, vals in groups.items():
        arr = np.array(sorted(vals), dtype=np.int64)
        if is_timestamp:
            arr = arr.view("datetime64[ns]")
        result[segment_id] = arr
    return result


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
