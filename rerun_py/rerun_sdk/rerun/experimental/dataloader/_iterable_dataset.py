"""IterableDataset backed by a catalog server."""

from __future__ import annotations

import functools
from typing import TYPE_CHECKING

import numpy as np
import torch
import torch.utils.data

from rerun._tracing import set_current_span_attributes, tracing_scope

from ._sample_index import FixedRateSampling, SampleIndex
from ._shuffle import SampleShuffle, ShuffleStrategy, _contiguous_shard, _fetch_chunks
from ._utils import (
    Target,
    _decode_iter,
    _decode_pool,
    _fetch_arrow,
    _fetch_targets,
    _interleave_fetch_and_decode,
    _replay,
    _resolve_decode_threads,
    _warn_if_fork_unsafe,
    _WorkerConnection,
)

if TYPE_CHECKING:
    from collections.abc import Generator, Iterator
    from concurrent.futures import ThreadPoolExecutor

    import pyarrow as pa

    from ._config import DataSource, Field
    from ._decoders import ColumnDecoder
    from .manifest._manifest import Manifest


def _count_yields(
    samples: Generator[dict[str, torch.Tensor | None], None, None],
) -> Generator[dict[str, torch.Tensor | None], None, None]:
    """
    Yield `samples` through, recording how many were yielded as a span attribute.

    The attribute is set when the stream ends — exhausted or closed early — so
    this must run inside the `tracing_scope` whose span should carry the count.
    Closes `samples` on exit: a plain `for` loop does not delegate `close()` to
    the source the way `yield from` does, and the fetch executor's shutdown
    hangs off that close.
    """
    count = 1
    try:
        for sample in samples:
            set_current_span_attributes({"rerun.dataloader.iter.num_samples_yielded": count})
            yield sample
            count += 1
    finally:
        samples.close()


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
        that determines the order samples are fetched in, and — for
        [`BlockShuffle`][rerun.experimental.dataloader.BlockShuffle] — the
        optional post-decode buffer samples are emitted through. Defaults to
        [`SampleShuffle`][rerun.experimental.dataloader.SampleShuffle]; pass
        [`NoShuffle`][rerun.experimental.dataloader.NoShuffle] for natural order.
    decode_threads
        Fields to decode concurrently within each `DataLoader` worker.

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
        decode_threads: int | None = None,
    ) -> None:
        super().__init__()

        _warn_if_fork_unsafe(stacklevel=3)

        self._fields = fields
        self._index = index
        self._fetch_size = fetch_size
        self._decode_threads = _resolve_decode_threads(decode_threads, fields)

        self._shuffle_strategy = shuffle_strategy if shuffle_strategy is not None else SampleShuffle()
        self._shuffle_buffer = self._shuffle_strategy.emission_buffer()
        self._epoch = 0
        self._manifest: Manifest | None = None

        self._sample_index = SampleIndex.build(
            source,
            index,
            self._fields,
            timeline_sampling=timeline_sampling,
        )

        self._connection = _WorkerConnection.from_source(source, fields)

    @classmethod
    def from_manifest(
        cls,
        manifest: Manifest,
        source: DataSource,
        fields: dict[str, Field],
        *,
        decode_threads: int | None = None,
    ) -> RerunIterableDataset:
        """
        Build a dataset that replays a frozen [`Manifest`][rerun.experimental.dataloader.Manifest]'s sampling order.

        The order, shards, and decode ranges all come from `manifest`; a manifest records only
        the field specs, not the objects, so the live connection and decoders are supplied here.

        Parameters
        ----------
        manifest:
            The frozen manifest to replay. Provides the sampling order, shards, and decode ranges.
        source:
            The live catalog connection.
        fields:
            The decoders, keyed by field name (a manifest records only their spec, not the objects).
        decode_threads:
            Fields to decode concurrently within each `DataLoader` worker.

        Returns
        -------
        RerunIterableDataset
            A dataset that yields samples in the manifest's recorded order.

        """
        self = cls.__new__(cls)
        torch.utils.data.IterableDataset.__init__(self)
        _warn_if_fork_unsafe(stacklevel=3)
        self._fields = fields
        self._index = manifest.metadata.index_name
        self._epoch = 0
        self._manifest = manifest
        self._decode_threads = _resolve_decode_threads(decode_threads, fields)
        self._connection = _WorkerConnection.from_source(source, fields)
        return self

    @property
    def sample_index(self) -> SampleIndex:
        """The underlying [`SampleIndex`][rerun.experimental.dataloader.SampleIndex]."""
        return self._sample_index

    def __len__(self) -> int:
        """Total number of samples across all segments."""
        if self._manifest is not None:
            return self._manifest.num_rows
        return self._sample_index.total_samples

    def set_epoch(self, epoch: int) -> None:
        """Set the epoch for shuffling (like `DistributedSampler.set_epoch`)."""
        self._epoch = epoch

    def __iter__(self) -> Iterator[dict[str, torch.Tensor | None]]:
        """Yield this worker's samples: replayed from a manifest, or fetched live from the catalog."""
        if self._manifest is not None:
            yield from self._iter_manifest()
        else:
            yield from self._iter_catalog()

    def _iter_catalog(self) -> Iterator[dict[str, torch.Tensor | None]]:
        """
        Yield individual samples as they are decoded.

        The arrow fetch for chunk N+1 runs on a background thread while
        chunk N is being decoded, so samples stream out during decode.
        When the strategy defines an emission buffer, decoded samples pass
        through that buffer before being yielded.
        """
        with (
            tracing_scope("RerunIterableDataset._iter_catalog"),
            _decode_pool(self._decode_threads, len(self._fields)) as executor,
        ):
            view, decoders = self._connection.ensure()

            indices, block_bounds = self._worker_order()
            chunks = _fetch_chunks(indices, block_bounds, fetch_size=self._fetch_size)

            set_current_span_attributes({
                "rerun.dataloader.iter.num_chunks": len(chunks),
                "rerun.dataloader.shuffle_strategy": self._shuffle_strategy.RECIPE_TAG,
            })

            fetch = functools.partial(
                _fetch_arrow,
                view=view,
                index=self._index,
                fields=self._fields,
                decoders=decoders,
                sample_index=self._sample_index,
            )

            samples = _interleave_fetch_and_decode(
                chunks, fetch=fetch, decode=functools.partial(self._decode, decoders, executor)
            )
            if self._shuffle_buffer is not None:
                distributed = torch.distributed.is_available() and torch.distributed.is_initialized()
                rank = torch.distributed.get_rank() if distributed else 0
                worker_info = torch.utils.data.get_worker_info()
                worker_id = worker_info.id if worker_info is not None else 0
                # Must match the build-time buffer seed in `_manifest_build._emit_rank`
                # (`[seed, rank, worker]`) so a manifest replays a live buffered run's exact order.
                rng = np.random.default_rng([self._epoch, rank, worker_id])
                samples = self._shuffle_buffer.shuffle(samples, rng=rng)
            yield from _count_yields(samples)

    def _iter_manifest(self) -> Iterator[dict[str, torch.Tensor | None]]:
        """Replay the manifest for this `(rank, worker)`: fetch groups in order, emit in the frozen order."""
        from .manifest._manifest_read import targets_from_rows

        assert self._manifest is not None
        with (
            tracing_scope("RerunIterableDataset._iter_manifest"),
            _decode_pool(self._decode_threads, len(self._fields)) as executor,
        ):
            view, decoders = self._connection.ensure()
            meta = self._manifest.metadata

            distributed = torch.distributed.is_available() and torch.distributed.is_initialized()
            rank = torch.distributed.get_rank() if distributed else 0
            world_size = torch.distributed.get_world_size() if distributed else 1
            worker_info = torch.utils.data.get_worker_info()
            worker = worker_info.id if worker_info is not None else 0
            num_workers = worker_info.num_workers if worker_info is not None else 1

            self._manifest.validate_topology(world_size, num_workers)
            # Read this worker's shard once (a disk hit for parquet-backed manifests) and
            # derive both the fetch schedule and the emission order from it.
            chunks, emit_order = self._manifest.worker_plan(rank, worker)
            if not chunks:
                set_current_span_attributes({
                    "rerun.dataloader.iter.num_samples_yielded": 0,
                    "rerun.dataloader.iter.num_chunks": len(chunks),
                })
                return

            set_current_span_attributes({
                "rerun.dataloader.iter.num_chunks": len(chunks),
                "rerun.dataloader.shuffle_strategy": self._manifest.metadata.shuffle_strategy,
            })

            # Query construction only needs the grid step / dtype, not the full sample space.
            sample_index = SampleIndex([], ns_per_sample=meta.ns_per_sample, ns_dtype=meta.ns_dtype)

            def fetch(chunk: pa.Table) -> tuple[list[Target], dict[str, dict[str, pa.Table]]]:
                targets = targets_from_rows(chunk, fields=self._fields, decoders=decoders, ns_dtype=meta.ns_dtype)
                return _fetch_targets(
                    targets,
                    view=view,
                    index=self._index,
                    fields=self._fields,
                    decoders=decoders,
                    sample_index=sample_index,
                )

            samples = _interleave_fetch_and_decode(
                chunks, fetch=fetch, decode=functools.partial(self._decode, decoders, executor)
            )
            yield from _count_yields(_replay(samples, emit_order))

    def _decode(
        self,
        decoders: dict[str, ColumnDecoder],
        executor: ThreadPoolExecutor | None,
        fetched: tuple[list[Target], dict[str, dict[str, pa.Table]]],
    ) -> Iterator[dict[str, torch.Tensor | None]]:
        """Decode one fetched chunk into samples (shared by the catalog and manifest paths)."""
        targets, seg_tables = fetched
        return _decode_iter(
            targets=targets,
            seg_tables=seg_tables,
            index=self._index,
            fields=self._fields,
            decoders=decoders,
            executor=executor,
        )

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
