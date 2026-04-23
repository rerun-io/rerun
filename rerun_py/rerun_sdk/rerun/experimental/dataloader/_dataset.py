"""IterableDataset backed by the Rerun Data Platform."""

from __future__ import annotations

import contextvars
import logging
import os
from collections import defaultdict
from concurrent.futures import Future, ThreadPoolExecutor
from typing import TYPE_CHECKING, Any, cast

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import torch
import torch.utils.data

from rerun._tracing import attach_parent_carrier, current_trace_carrier, tracing_scope, with_tracing
from rerun.catalog import CatalogClient

from ._sample_index import FixedRateSampling, SampleIndex

if TYPE_CHECKING:
    from collections.abc import Iterator

    from ._config import Column, DataSource
    from ._decoders import ColumnDecoder
    from ._sample_index import SegmentMetadata

logger = logging.getLogger(__name__)

#: (segment_metadata, index_value) pair identifying one sample to produce.
Target = tuple["SegmentMetadata", "int | np.datetime64"]


class RerunDataset(torch.utils.data.IterableDataset[dict[str, torch.Tensor]]):
    """
    Iterable dataset backed by the Rerun Data Platform.

    Internally fetches data in large chunks (`fetch_size` samples per
    server query) and yields individual samples. This amortizes the
    fixed per-query overhead over many samples while letting the
    `DataLoader` control the training batch size independently.

    Shuffling is handled internally: each epoch shuffles the full index
    list, then partitions it across workers. Use `set_epoch` to
    re-seed the shuffle between epochs.

    Parameters
    ----------
    source
        The dataset to read from (with optional segment filter).
    index
        Timeline to iterate (e.g. `"frame_nr"`).
    columns
        Output fields, keyed by output name.
    timeline_sampling
        Required when `index` is a timestamp timeline; ignored for
        integer indices. Pass [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling] to sample on
        a fixed grid (e.g. 30 Hz).
    fetch_size
        Number of samples to fetch per server query. Larger values
        amortize network overhead but use more memory. Defaults to 128.
    shuffle
        Whether to shuffle sample order each epoch. Defaults to True.
    token
        Authentication token for worker reconnection.

    Examples
    --------
    ```python
    dataset = RerunDataset(
        source,
        "frame_nr",
        {"image": Column("/camera:Image:blob", decode=ImageDecoder())},
        fetch_size=256,
    )
    loader = DataLoader(dataset, batch_size=8, num_workers=4)
    for batch in loader:
        ...
    ```

    """

    def __init__(
        self,
        source: DataSource,
        index: str,
        columns: dict[str, Column],
        *,
        timeline_sampling: FixedRateSampling | None = None,
        fetch_size: int = 128,
        shuffle: bool = True,
        token: str | None = None,
    ) -> None:
        super().__init__()

        self._columns = columns
        self._index = index
        self._fetch_size = fetch_size
        self._shuffle = shuffle
        self._epoch = 0

        self._sample_index = SampleIndex.build(
            source,
            index,
            self._columns,
            timeline_sampling=timeline_sampling,
        )

        seg_sizes = np.array([s.num_samples for s in self._sample_index.segments], dtype=np.int64)
        self._cumulative_sizes = np.concatenate([[0], np.cumsum(seg_sizes)])

        self._catalog_url = source.dataset.catalog.url
        self._dataset_name = source.dataset.name
        self._token = token

        # Per-worker state, lazily initialized.
        self._initialized: bool = False
        self._init_pid: int = -1
        self._decoders: dict[str, ColumnDecoder] = {}
        self._view: Any = None

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

    @property
    def sample_index(self) -> SampleIndex:
        """The underlying [`SampleIndex`][rerun.experimental.dataloader.SampleIndex] — useful for diagnostics."""
        return self._sample_index

    def __len__(self) -> int:
        """Total number of samples across all segments."""
        return self._sample_index.total_samples

    def set_epoch(self, epoch: int) -> None:
        """Set the epoch for shuffling (like `DistributedSampler.set_epoch`)."""
        self._epoch = epoch

    def __iter__(self) -> Iterator[dict[str, torch.Tensor]]:
        """
        Yield individual samples as they're decoded.

        Pipeline: the arrow fetch for chunk N+1 runs on a background
        thread while chunk N is being decoded and yielded, so samples
        stream out during decode instead of waiting for the full chunk.
        """
        self._ensure_initialized()

        indices = self._worker_indices()
        chunks = [indices[i : i + self._fetch_size] for i in range(0, len(indices), self._fetch_size)]

        if not chunks:
            return

        executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix="rerun-fetch")

        def submit_fetch(chunk: np.ndarray) -> Future[tuple[list[Target], dict[str, pa.Table]]]:
            # Copy the calling thread's contextvars so _fetch_arrow's span is
            # parented under the current OTel context instead of appearing as a root trace.
            ctx = contextvars.copy_context()
            return cast(
                "Future[tuple[list[Target], dict[str, pa.Table]]]",
                executor.submit(ctx.run, self._fetch_arrow, chunk),
            )

        try:
            pending: Future[tuple[list[Target], dict[str, pa.Table]]] | None = submit_fetch(chunks[0])
            for i, _ in enumerate(chunks):
                assert pending is not None
                targets, seg_tables = pending.result()
                pending = submit_fetch(chunks[i + 1]) if i + 1 < len(chunks) else None
                yield from self._decode_iter(targets, seg_tables)
        finally:
            with tracing_scope("executor.shutdown"):
                executor.shutdown(wait=False)

    def _worker_indices(self) -> np.ndarray:
        """Return the indices this worker is responsible for, shuffled if requested."""
        all_indices = np.arange(self._sample_index.total_samples)

        if self._shuffle:
            rng = np.random.default_rng(seed=self._epoch)
            rng.shuffle(all_indices)

        # Partition across distributed ranks first (DDP), then across
        # DataLoader workers within this rank. Contiguous blocks (not
        # interleaved) so workers hit their fetch boundaries at different
        # times.
        if torch.distributed.is_available() and torch.distributed.is_initialized():
            all_indices = _contiguous_shard(
                all_indices,
                rank=torch.distributed.get_rank(),
                world_size=torch.distributed.get_world_size(),
            )

        worker_info = torch.utils.data.get_worker_info()
        if worker_info is not None:
            all_indices = _contiguous_shard(
                all_indices,
                rank=worker_info.id,
                world_size=worker_info.num_workers,
            )

        return all_indices

    @with_tracing("RerunDataset._fetch_arrow")
    def _fetch_arrow(self, indices: np.ndarray) -> tuple[list[Target], dict[str, pa.Table]]:
        """
        Run the server query for `indices` and return `(targets, per-segment tables)`.

        Called on a background thread so the next chunk's arrow fetch
        overlaps with the current chunk's decode.
        """
        assert self._view is not None

        targets: list[Target] = [self._global_to_local(idx) for idx in indices]
        query_indices = _build_query_indices(
            targets,
            self._columns,
            self._decoders,
            sample_index=self._sample_index,
        )

        reader = self._view.reader(
            index=self._index,
            using_index_values=query_indices,
            fill_latest_at=True,
        )
        arrow_table = reader.to_arrow_table()
        seg_tables = _split_by_segment(arrow_table)

        return targets, seg_tables

    def _decode_iter(
        self,
        targets: list[Target],
        seg_tables: dict[str, pa.Table],
    ) -> Iterator[dict[str, torch.Tensor]]:
        """Yield decoded samples one at a time from a pre-fetched arrow chunk."""
        for seg_meta, idx_val in targets:
            with tracing_scope("RerunDataset._decode_sample"):
                seg_table = seg_tables.get(seg_meta.segment_id)
                if seg_table is None:
                    raise RuntimeError(f"No rows returned for segment {seg_meta.segment_id!r} at index {idx_val!r}")
                sample: dict[str, torch.Tensor] = {}
                index_array = seg_table[self._index]
                for key, col in self._columns.items():
                    decoder = self._decoders[key]
                    lo, hi = _column_index_range(idx_val, col, decoder) or (idx_val, idx_val)
                    mask = pc.and_(
                        pc.greater_equal(index_array, lo),
                        pc.less_equal(index_array, hi),
                    )
                    raw = seg_table.filter(mask).column(col.path)
                    sample[key] = decoder.decode(raw, idx_val, seg_meta.segment_id)
            yield sample

    def _global_to_local(self, idx: int) -> tuple[SegmentMetadata, int | np.datetime64]:
        """Map a global index `[0, total_samples)` to `(segment, concrete_idx_value)`."""
        if idx < 0 or idx >= self._sample_index.total_samples:
            raise IndexError(f"Index {idx} out of range [0, {self._sample_index.total_samples})")
        seg_idx = int(np.searchsorted(self._cumulative_sizes[1:], idx, side="right"))
        pos = idx - int(self._cumulative_sizes[seg_idx])
        seg = self._sample_index.segments[seg_idx]
        return seg, self._sample_index.resolve_local_index(seg, pos)

    @with_tracing("RerunDataset._ensure_initialized")
    def _ensure_initialized(self) -> None:
        """Lazily set up per-worker catalog connection, decoders, and view."""
        pid = os.getpid()
        if self._initialized and self._init_pid == pid:
            return

        client = CatalogClient(self._catalog_url, token=self._token)
        dataset = client.get_dataset(self._dataset_name)
        self._decoders = {k: col.decode for k, col in self._columns.items()}
        self._view = dataset.filter_contents(_derive_content_filter(self._columns))

        self._initialized = True
        self._init_pid = pid


def _contiguous_shard(indices: np.ndarray, *, rank: int, world_size: int) -> np.ndarray:
    """Return the `rank`-th contiguous slice of `indices`, with the last rank taking the remainder."""
    per_shard = len(indices) // world_size
    start = rank * per_shard
    end = start + per_shard if rank < world_size - 1 else len(indices)
    return indices[start:end]


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
        sid = seg_meta.segment_id
        iv_int = int(idx_val)

        groups[sid].add(iv_int)

        for key, col in columns.items():
            rng = _column_index_range(idx_val, col, decoders[key])
            if rng is None:
                continue
            lo, hi = rng
            for val in sample_index.indices_in_range(seg_meta, int(lo), int(hi)):
                groups[sid].add(int(val))

    result: dict[str, np.ndarray] = {}
    for sid, vals in groups.items():
        arr = np.array(sorted(vals), dtype=np.int64)
        if is_timestamp:
            arr = arr.view("datetime64[ns]")
        result[sid] = arr
    return result


def _split_by_segment(table: pa.Table) -> dict[str, pa.Table]:
    """Split a combined table into per-segment tables."""
    seg_col = table.column("rerun_segment_id")
    return {sid.as_py(): table.filter(pc.equal(seg_col, sid)) for sid in pc.unique(seg_col)}


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
