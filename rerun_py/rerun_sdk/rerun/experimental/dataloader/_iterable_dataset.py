"""IterableDataset backed by a catalog server."""

from __future__ import annotations

import contextvars
from concurrent.futures import Future, ThreadPoolExecutor
from typing import TYPE_CHECKING

import numpy as np
import torch
import torch.utils.data

from rerun._tracing import tracing_scope

from ._sample_index import FixedRateSampling, SampleIndex
from ._shuffle import SampleShuffle, ShuffleBuffer, ShuffleStrategy, _contiguous_shard, _fetch_chunks
from ._utils import Target, _decode_iter, _fetch_arrow, _warn_if_fork_unsafe, _WorkerConnection

if TYPE_CHECKING:
    from collections.abc import Generator, Iterator

    import pyarrow as pa

    from ._config import DataSource, Field


class RerunIterableDataset(torch.utils.data.IterableDataset[dict[str, torch.Tensor | None]]):
    """
    Iterable dataset backed by a catalog server.

    Fetches `fetch_size` samples per server query and yields individual
    samples, so per-query overhead is amortized across many samples while
    the `DataLoader` controls the training batch size independently.

    The index list is partitioned across DDP ranks and DataLoader workers
    internally. With shuffling enabled (default), the sample order is permuted
    once per epoch before partitioning; call `set_epoch` to re-seed between
    epochs.

    Parameters
    ----------
    source
        The dataset to read from (with optional segment filter).
    index
        Timeline to iterate (e.g. `"frame_nr"`).
    fields
        Sample fields, keyed by output name.
    timeline_sampling
        Required when `index` is a timestamp timeline; ignored for
        integer indices. Pass [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling]
        to sample on a fixed grid (e.g. 30 Hz).
    fetch_size
        Number of samples to fetch per server query. Larger values
        amortize network overhead but use more memory. Defaults to 128.
    shuffle_strategy
        The [`ShuffleStrategy`][rerun.experimental.dataloader.ShuffleStrategy]
        that determines the order samples are fetched in. Defaults to
        [`SampleShuffle`][rerun.experimental.dataloader.SampleShuffle]; pass
        [`NoShuffle`][rerun.experimental.dataloader.NoShuffle] for natural order.
    shuffle_buffer_size
        Size of a post-decode shuffle buffer
        that randomizes emission order without changing the fetch order;
        mainly useful to decorrelate batches under
        [`BlockShuffle`][rerun.experimental.dataloader.BlockShuffle]. Holds at
        most that many decoded samples in memory per DataLoader worker.
        Must be at least `fetch_size`, so the buffer can absorb a whole fetch.
        Defaults to `None` (no buffering).

    """

    def __init__(
        self,
        source: DataSource,
        index: str,
        fields: dict[str, Field],
        *,
        timeline_sampling: FixedRateSampling | None = None,
        fetch_size: int = 128,
        shuffle_strategy: ShuffleStrategy | None = None,
        shuffle_buffer_size: int | None = None,
    ) -> None:
        super().__init__()

        _warn_if_fork_unsafe(stacklevel=3)

        self._fields = fields
        self._index = index
        self._fetch_size = fetch_size

        self._shuffle_strategy = shuffle_strategy if shuffle_strategy is not None else SampleShuffle()
        if shuffle_buffer_size is not None and shuffle_buffer_size < fetch_size:
            raise ValueError(
                f"shuffle_buffer_size must be at least fetch_size ({fetch_size}), got {shuffle_buffer_size}"
            )
        self._shuffle_buffer = ShuffleBuffer(shuffle_buffer_size) if shuffle_buffer_size is not None else None
        self._epoch = 0

        self._sample_index = SampleIndex.build(
            source,
            index,
            self._fields,
            timeline_sampling=timeline_sampling,
        )

        self._connection = _WorkerConnection(
            catalog_url=source.dataset.catalog.url,
            dataset_name=source.dataset.name,
            fields=fields,
        )

    @property
    def sample_index(self) -> SampleIndex:
        """The underlying [`SampleIndex`][rerun.experimental.dataloader.SampleIndex]."""
        return self._sample_index

    def __len__(self) -> int:
        """Total number of samples across all segments."""
        return self._sample_index.total_samples

    def set_epoch(self, epoch: int) -> None:
        """Set the epoch for shuffling (like `DistributedSampler.set_epoch`)."""
        self._epoch = epoch

    def __iter__(self) -> Iterator[dict[str, torch.Tensor | None]]:
        """
        Yield individual samples as they are decoded.

        The arrow fetch for chunk N+1 runs on a background thread while
        chunk N is being decoded, so samples stream out during decode.
        With `shuffle_buffer_size` set, decoded samples pass through a
        shuffle buffer before being yielded.
        """
        with tracing_scope("RerunIterableDataset.__iter__"):
            view, decoders = self._connection.ensure()

            indices, block_bounds = self._worker_order()
            chunks = _fetch_chunks(indices, block_bounds, fetch_size=self._fetch_size)

            if not chunks:
                return

            def fetch_and_decode() -> Generator[dict[str, torch.Tensor | None], None, None]:
                executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix="rerun-fetch")

                def submit_fetch(chunk: np.ndarray) -> Future[tuple[list[Target], dict[str, dict[str, pa.Table]]]]:
                    # Copy the calling thread's contextvars so _fetch_arrow's span is
                    # parented under the current OTel context instead of appearing as a root trace.
                    ctx = contextvars.copy_context()

                    def fetch() -> tuple[list[Target], dict[str, dict[str, pa.Table]]]:
                        return ctx.run(
                            _fetch_arrow,
                            view=view,
                            index=self._index,
                            fields=self._fields,
                            decoders=decoders,
                            sample_index=self._sample_index,
                            indices=chunk,
                        )

                    return executor.submit(fetch)

                try:
                    pending: Future[tuple[list[Target], dict[str, dict[str, pa.Table]]]] | None = submit_fetch(
                        chunks[0]
                    )
                    for i, _ in enumerate(chunks):
                        assert pending is not None
                        targets, seg_tables = pending.result()
                        pending = submit_fetch(chunks[i + 1]) if i + 1 < len(chunks) else None
                        yield from _decode_iter(
                            targets=targets,
                            seg_tables=seg_tables,
                            index=self._index,
                            fields=self._fields,
                            decoders=decoders,
                        )
                finally:
                    with tracing_scope("executor.shutdown"):
                        executor.shutdown(wait=False)

            samples = fetch_and_decode()
            if self._shuffle_buffer is not None:
                worker_info = torch.utils.data.get_worker_info()
                worker_id = worker_info.id if worker_info is not None else 0
                rng = np.random.default_rng([self._epoch, worker_id])
                samples = self._shuffle_buffer.shuffle(samples, rng=rng)
            yield from samples

    def _worker_order(self) -> tuple[np.ndarray, np.ndarray]:
        """Return this worker's shard of the epoch's `(indices, block_bounds)`, in fetch order."""
        indices, block_bounds = self._shuffle_strategy.epoch_order(
            self._sample_index,
            fetch_size=self._fetch_size,
            seed=self._epoch,
        )

        # Partition across distributed ranks first (DDP), then across
        # DataLoader workers within this rank. Contiguous (not interleaved)
        # slices keep a worker on a small set of segments.
        if torch.distributed.is_available() and torch.distributed.is_initialized():
            indices, block_bounds = _contiguous_shard(
                indices,
                block_bounds,
                rank=torch.distributed.get_rank(),
                world_size=torch.distributed.get_world_size(),
            )

        worker_info = torch.utils.data.get_worker_info()
        if worker_info is not None:
            indices, block_bounds = _contiguous_shard(
                indices,
                block_bounds,
                rank=worker_info.id,
                world_size=worker_info.num_workers,
            )

        return indices, block_bounds
