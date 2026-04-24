"""IterableDataset backed by the Rerun Data Platform."""

from __future__ import annotations

import contextvars
from concurrent.futures import Future, ThreadPoolExecutor
from typing import TYPE_CHECKING

import numpy as np
import torch
import torch.utils.data

from rerun._tracing import tracing_scope

from ._sample_index import FixedRateSampling, SampleIndex
from ._utils import Target, _decode_iter, _fetch_arrow, _WorkerConnection

if TYPE_CHECKING:
    from collections.abc import Iterator

    import pyarrow as pa

    from ._config import Column, DataSource


class RerunIterableDataset(torch.utils.data.IterableDataset[dict[str, torch.Tensor]]):
    """
    Iterable dataset backed by the Rerun Data Platform.

    Internally fetches data in large chunks (`fetch_size` samples per server query) and yields individual samples.
    This amortizes the fixed per-query overhead over many samples while letting the `DataLoader` control the training batch size independently.

    Shuffling is handled internally: each epoch shuffles the full index list, then partitions it across workers.
    Use `set_epoch` to re-seed the shuffle between epochs.

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

        self._connection = _WorkerConnection(
            catalog_url=source.dataset.catalog.url,
            dataset_name=source.dataset.name,
            columns=columns,
        )

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
        with tracing_scope("RerunIterableDataset.__iter__"):
            view, decoders = self._connection.ensure()

            indices = self._worker_indices()
            chunks = [indices[i : i + self._fetch_size] for i in range(0, len(indices), self._fetch_size)]

            if not chunks:
                return

            executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix="rerun-fetch")

            def submit_fetch(chunk: np.ndarray) -> Future[tuple[list[Target], dict[str, pa.Table]]]:
                # Copy the calling thread's contextvars so _fetch_arrow's span is
                # parented under the current OTel context instead of appearing as a root trace.
                ctx = contextvars.copy_context()

                def fetch() -> tuple[list[Target], dict[str, pa.Table]]:
                    return ctx.run(
                        _fetch_arrow,
                        view=view,
                        index=self._index,
                        columns=self._columns,
                        decoders=decoders,
                        sample_index=self._sample_index,
                        indices=chunk,
                    )

                return executor.submit(fetch)

            try:
                pending: Future[tuple[list[Target], dict[str, pa.Table]]] | None = submit_fetch(chunks[0])
                for i, _ in enumerate(chunks):
                    assert pending is not None
                    targets, seg_tables = pending.result()
                    pending = submit_fetch(chunks[i + 1]) if i + 1 < len(chunks) else None
                    yield from _decode_iter(
                        targets=targets,
                        seg_tables=seg_tables,
                        index=self._index,
                        columns=self._columns,
                        decoders=decoders,
                    )
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


def _contiguous_shard(indices: np.ndarray, *, rank: int, world_size: int) -> np.ndarray:
    """Return the `rank`-th contiguous slice of `indices`, with the last rank taking the remainder."""
    per_shard = len(indices) // world_size
    start = rank * per_shard
    end = start + per_shard if rank < world_size - 1 else len(indices)
    return indices[start:end]
