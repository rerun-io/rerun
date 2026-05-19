"""Shared helpers used by both the iterable and map-style Rerun datasets."""

from __future__ import annotations

import os
from collections import defaultdict
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow.compute as pc

from rerun._tracing import attach_parent_carrier, current_trace_carrier, tracing_scope, with_tracing
from rerun.catalog import CatalogClient

if TYPE_CHECKING:
    from collections.abc import Iterator

    import pyarrow as pa
    import torch

    from ._config import Column
    from ._decoders import ColumnDecoder
    from ._sample_index import SampleIndex, SegmentMetadata

#: (segment_metadata, index_value) pair identifying one sample to produce.
Target = tuple["SegmentMetadata", "int | np.datetime64"]


class _WorkerConnection:
    """Lazily-initialized per-worker catalog connection, view, and decoders."""

    def __init__(
        self,
        *,
        catalog_url: str,
        dataset_name: str,
        columns: dict[str, Column],
    ) -> None:
        self._catalog_url = catalog_url
        self._dataset_name = dataset_name
        self._columns = columns
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
        self._decoders = {k: col.decode for k, col in self._columns.items()}
        self._view = dataset.filter_contents(_derive_content_filter(self._columns))
        self._initialized = True
        self._init_pid = pid
        return self._view, self._decoders

    def __getstate__(self) -> dict[str, Any]:
        """Strip the unpicklable catalog view so DataLoader can send us to workers."""
        state = self.__dict__.copy()
        state["_view"] = None
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
    columns: dict[str, Column],
    decoders: dict[str, ColumnDecoder],
    sample_index: SampleIndex,
    indices: np.ndarray | list[int],
) -> tuple[list[Target], dict[str, pa.Table]]:
    """Run the server query for `indices` and return `(targets, per-segment tables)`."""
    targets: list[Target] = [sample_index.global_to_local(int(idx)) for idx in indices]
    query_indices = _build_query_indices(
        targets,
        columns,
        decoders,
        sample_index=sample_index,
    )

    reader = view.reader(
        index=index,
        using_index_values=query_indices,
        fill_latest_at=True,
    )
    arrow_table = reader.to_arrow_table()
    seg_tables = _split_by_segment(arrow_table)

    return targets, seg_tables


def _decode_iter(
    *,
    targets: list[Target],
    seg_tables: dict[str, pa.Table],
    index: str,
    columns: dict[str, Column],
    decoders: dict[str, ColumnDecoder],
) -> Iterator[dict[str, torch.Tensor]]:
    """Yield decoded samples one at a time from a pre-fetched arrow chunk."""
    with tracing_scope("RerunDataset._decode_chunk"):
        for seg_meta, idx_val in targets:
            with tracing_scope("RerunDataset._decode_sample"):
                seg_table = seg_tables.get(seg_meta.segment_id)
                if seg_table is None:
                    raise RuntimeError(f"No rows returned for segment {seg_meta.segment_id!r} at index {idx_val!r}")
                sample: dict[str, torch.Tensor] = {}
                index_array = seg_table[index]
                for key, col in columns.items():
                    decoder = decoders[key]
                    lo, hi = _column_index_range(idx_val, col, decoder) or (idx_val, idx_val)
                    mask = pc.and_(
                        pc.greater_equal(index_array, lo),
                        pc.less_equal(index_array, hi),
                    )
                    raw = seg_table.filter(mask).column(col.path)
                    sample[key] = decoder.decode(raw, idx_val, seg_meta.segment_id)
            yield sample


def _column_index_range(
    idx_val: int | np.datetime64,
    col: Column,
    decoder: ColumnDecoder,
) -> tuple[Any, Any] | None:
    """
    Inclusive `(lo, hi)` range of index values needed for one column at `idx_val`, or `None` if only `idx_val` itself is needed.

    Window (e.g. action windows) takes precedence over decoder context
    (e.g. video keyframe prefetch).
    """
    if col.window is not None:
        return idx_val + col.window[0], idx_val + col.window[1]
    return decoder.context_range(idx_val)


def _build_query_indices(
    targets: list[Target],
    columns: dict[str, Column],
    decoders: dict[str, ColumnDecoder],
    *,
    sample_index: SampleIndex,
) -> dict[str, np.ndarray]:
    """
    Group targets by segment and expand with window + decoder context.

    Returns a `{segment_id: ndarray_of_index_values}` dict ready for
    `reader(using_index_values=…)`. Values are `int64` for integer
    indices and `datetime64[ns]` for timestamp timelines.
    """
    is_timestamp = sample_index.is_timestamp
    groups: dict[str, set[int]] = defaultdict(set)

    for seg_meta, idx_val in targets:
        segment_id = seg_meta.segment_id

        groups[segment_id].add(int(idx_val))

        for key, col in columns.items():
            rng = _column_index_range(idx_val, col, decoders[key])
            if rng is None:
                continue
            lo, hi = rng
            for val in sample_index.indices_in_range(seg_meta, int(lo), int(hi)):
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


def _derive_content_filter(columns: dict[str, Column]) -> list[str]:
    """
    Build content-filter patterns from column paths.

    `"/camera:EncodedImage:blob"` → `"/camera/**"`
    """
    paths: set[str] = set()
    for col in columns.values():
        entity = col.path.split(":")[0]
        paths.add(f"{entity}/**")
    return sorted(paths)
