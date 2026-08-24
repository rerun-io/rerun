"""IterableDataset backed by a catalog server."""

from __future__ import annotations

import time
import warnings
from typing import TYPE_CHECKING

import numpy as np
import torch
import torch.utils.data

from rerun._tracing import set_current_span_attributes, tracing_scope, with_tracing

from ._sample_index import FixedRateSampling, SampleIndex
from ._shuffle import SampleShuffle, ShuffleStrategy, _contiguous_shard, _fetch_blocks
from ._utils import (
    FetchedBlock,
    _build_query_plans,
    _build_targets,
    _decode_iter,
    _decode_pool,
    _fetch_prior_keyframes,
    _fetch_queries_parallel,
    _index_fetched_block,
    _locate_samples,
    _pipeline_blocks,
    _replay,
    _resolve_decode_requests_in_block,
    _resolve_decode_threads,
    _warn_if_fork_unsafe,
    _WorkerConnection,
)
from .decoders._base import DecodedSample

if TYPE_CHECKING:
    from collections.abc import Generator, Iterator

    import pyarrow as pa

    from ._config import DataSource, Field
    from ._utils import Target
    from .manifest._manifest import Manifest


_DEFAULT_MAX_CONSECUTIVE_SKIPPED_SAMPLES = 1000


def _skip_incomplete(
    samples: Generator[DecodedSample, None, None],
    *,
    max_consecutive_skipped_samples: int | None = _DEFAULT_MAX_CONSECUTIVE_SKIPPED_SAMPLES,
) -> Generator[DecodedSample, None, None]:
    """Drop live samples with missing fields up to the configured limit."""
    if max_consecutive_skipped_samples is not None and max_consecutive_skipped_samples < 0:
        raise ValueError(f"max_consecutive_skipped_samples must be non-negative, got {max_consecutive_skipped_samples}")

    skipped = 0
    consecutive_skipped = 0
    skipped_by_field: dict[str, int] = {}
    warned: set[str] = set()
    try:
        for sample in samples:
            missing = [key for key, value in sample.items() if value is None]
            if not missing:
                consecutive_skipped = 0
                yield sample
                continue

            skipped += 1
            consecutive_skipped += 1
            for key in missing:
                skipped_by_field[key] = skipped_by_field.get(key, 0) + 1
            set_current_span_attributes({"rerun.dataloader.iter.num_samples_skipped": skipped})
            for key in sorted(set(missing) - warned):
                warnings.warn(
                    f"Skipping samples where field {key!r} has no value. "
                    "Batches stay at full size; the epoch yields fewer samples.",
                    RuntimeWarning,
                    stacklevel=2,
                )
            warned.update(missing)
            if max_consecutive_skipped_samples is not None and consecutive_skipped > max_consecutive_skipped_samples:
                field_counts = ", ".join(f"{key}={skipped_by_field[key]}" for key in sorted(skipped_by_field))
                raise RuntimeError(
                    f"Exceeded max_consecutive_skipped_samples={max_consecutive_skipped_samples} after "
                    f"encountering {consecutive_skipped} consecutive incomplete samples "
                    f"({skipped} total; missing fields: {field_counts})"
                )
    finally:
        samples.close()


def _raise_if_incomplete(
    sample: DecodedSample,
    target: Target,
    required: set[str],
) -> None:
    """Raise if a required field is missing despite a manifest's validation."""
    missing = [key for key in required if sample.get(key) is None]
    if missing:
        raise RuntimeError(
            f"Required fields decoded to nothing: {', '.join(sorted(missing))}. The manifest was built "
            "against different data, so regenerate it.\n"
            f"Segment: {target.segment.segment_id} at {target.index_value}"
        )


def _count_yields(
    samples: Generator[DecodedSample, None, None],
) -> Generator[DecodedSample, None, None]:
    """
    Yield `samples` through, recording the yield count and downstream pull gaps on the current span.

    A pull gap is the wall-clock time from yielding a sample until the consumer resumes the generator.
    Closes `samples` on exit: a plain `for` loop does not delegate `close()` to
    the source the way `yield from` does, and the fetch executor's shutdown
    hangs off that close.
    """
    count = 1
    pull_gap_seconds_total = 0.0
    pull_gap_seconds_max = 0.0
    try:
        for sample in samples:
            set_current_span_attributes({"rerun.dataloader.iter.num_samples_yielded": count})
            before_yield = time.perf_counter()
            try:
                yield sample
            finally:
                pull_gap_seconds = time.perf_counter() - before_yield
                pull_gap_seconds_total += pull_gap_seconds
                pull_gap_seconds_max = max(pull_gap_seconds_max, pull_gap_seconds)
                set_current_span_attributes({
                    "rerun.dataloader.iter.pull_gap_seconds_total": pull_gap_seconds_total,
                    "rerun.dataloader.iter.pull_gap_seconds_max": pull_gap_seconds_max,
                })
            count += 1
    finally:
        samples.close()


class RerunIterableDataset(torch.utils.data.IterableDataset[DecodedSample]):
    """
    Iterable dataset backed by a catalog server.

    Fetches `fetch_block_size` samples per server query and yields individual
    samples, so per-query overhead is amortized across many samples while
    the `DataLoader` controls the training batch size independently.

    The index list is partitioned across DDP ranks and DataLoader workers
    internally. With shuffling enabled (default), the sample order is permuted
    once per epoch before partitioning; call `set_epoch` to re-seed between
    epochs. When a finite live dataset is consumed to exhaustion under DDP,
    wrap the training loop in `DistributedDataParallel.join()`: rank shards can
    have different lengths, especially when incomplete samples are skipped
    after sharding.

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
    fetch_block_size
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
    max_consecutive_skipped_samples
        Maximum number of consecutive incomplete samples to skip in each live
        iterator, independently for every rank and `DataLoader` worker. A valid
        sample resets the count. The next missing sample raises with total and
        per-field counts. Defaults to 1000; pass `None` to apply no limit.
        Manifest replay remains strict regardless of this setting.

    """

    def __init__(
        self,
        source: DataSource,
        index: str,
        fields: dict[str, Field],
        *,
        timeline_sampling: FixedRateSampling | None = None,
        fetch_block_size: int = 128,
        shuffle_strategy: ShuffleStrategy | None = None,
        decode_threads: int | None = None,
        max_consecutive_skipped_samples: int | None = _DEFAULT_MAX_CONSECUTIVE_SKIPPED_SAMPLES,
    ) -> None:
        super().__init__()

        _warn_if_fork_unsafe(stacklevel=3)
        if max_consecutive_skipped_samples is not None and max_consecutive_skipped_samples < 0:
            raise ValueError(
                f"max_consecutive_skipped_samples must be non-negative, got {max_consecutive_skipped_samples}"
            )

        self._fields = fields
        self._index = index
        self._fetch_block_size = fetch_block_size
        self._decode_threads = _resolve_decode_threads(decode_threads, fields)
        self._max_consecutive_skipped_samples = max_consecutive_skipped_samples

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

    def __iter__(self) -> Iterator[DecodedSample]:
        """Yield this worker's samples: replayed from a manifest, or fetched live from the catalog."""
        if self._manifest is not None:
            yield from self._iter_manifest()
        else:
            yield from self._iter_catalog()

    def _iter_catalog(self) -> Iterator[DecodedSample]:
        """
        Yield individual samples as they are decoded.

        The arrow fetch for block N+1 runs on a background thread while
        block N is being decoded, so samples stream out during decode.
        When the strategy defines an emission buffer, decoded samples pass
        through that buffer before being yielded.
        """
        with (
            tracing_scope("RerunIterableDataset._iter_catalog"),
            _decode_pool(self._decode_threads, len(self._fields)) as executor,
        ):
            view, decoders = self._connection.ensure()

            indices, block_bounds = self._worker_order()
            blocks = _fetch_blocks(indices, block_bounds, fetch_block_size=self._fetch_block_size)

            set_current_span_attributes({
                "rerun.dataloader.iter.num_blocks": len(blocks),
                "rerun.dataloader.shuffle_strategy": self._shuffle_strategy.RECIPE_TAG,
            })

            @with_tracing("RerunDataset._fetch_block")
            def fetch_block(block: np.ndarray) -> FetchedBlock:
                # 1. Locate requested samples within their segments.
                located = _locate_samples(
                    block,
                    sample_index=self._sample_index,
                    num_fields=len(self._fields),
                )
                # 2. Fetch the prior-keyframe metadata needed by compressed-video requests.
                keyframes = _fetch_prior_keyframes(
                    view=view,
                    index=self._index,
                    fields=self._fields,
                    located=located,
                    sample_index=self._sample_index,
                )
                # 3. Match samples with prior video keyframes and compute each field's index ranges.
                targets = _build_targets(
                    located,
                    keyframes,
                    fields=self._fields,
                    sample_index=self._sample_index,
                )
                # 4. Aggregate compatible field requests into server queries.
                query_plans = _build_query_plans(targets, self._fields, sample_index=self._sample_index)
                # 5. Execute independent server queries concurrently.
                fetched_groups = _fetch_queries_parallel(query_plans, view=view, index=self._index)
                return FetchedBlock(targets=targets, fetched_groups=fetched_groups)

            def process(fetched: FetchedBlock) -> Iterator[DecodedSample]:
                # 1. Group each fetched table into contiguous, index-ordered segment spans.
                indexed = _index_fetched_block(fetched, self._index)
                # 2. Map each requested timeline range to physical Arrow rows.
                prepared = _resolve_decode_requests_in_block(indexed)
                # 3. Decode field batches and scatter their results back into sample order.
                return _decode_iter(
                    prepared=prepared,
                    decoders=decoders,
                    executor=executor,
                )

            samples = _skip_incomplete(
                _pipeline_blocks(blocks, fetch=fetch_block, process=process),
                max_consecutive_skipped_samples=self._max_consecutive_skipped_samples,
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

    def _iter_manifest(self) -> Iterator[DecodedSample]:
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
            blocks, emit_order = self._manifest.worker_plan(rank, worker)
            if not blocks:
                set_current_span_attributes({
                    "rerun.dataloader.iter.num_samples_yielded": 0,
                    "rerun.dataloader.iter.num_blocks": len(blocks),
                })
                return

            set_current_span_attributes({
                "rerun.dataloader.iter.num_blocks": len(blocks),
                "rerun.dataloader.shuffle_strategy": self._manifest.metadata.shuffle_strategy,
            })

            # Query construction only needs the grid step / dtype, not the full sample space.
            sample_index = SampleIndex([], ns_per_sample=meta.ns_per_sample, ns_dtype=meta.ns_dtype)

            @with_tracing("RerunDataset._fetch_block")
            def fetch_block(block: pa.Table) -> FetchedBlock:
                targets = targets_from_rows(block, fields=self._fields, sample_index=sample_index)
                set_current_span_attributes({"rerun.dataloader.fetch.block_size": len(targets)})
                query_plans = _build_query_plans(targets, self._fields, sample_index=sample_index)
                fetched_groups = _fetch_queries_parallel(query_plans, view=view, index=self._index)
                return FetchedBlock(targets=targets, fetched_groups=fetched_groups)

            # Only the fields being decoded: a replay may ask for a subset of what was frozen.
            required = {key for key in meta.required_fields if key in self._fields}

            def process(fetched: FetchedBlock) -> Iterator[DecodedSample]:
                # 1. Group each fetched table into contiguous, index-ordered segment spans.
                indexed = _index_fetched_block(fetched, self._index)
                # 2. Map each requested timeline range to physical Arrow rows.
                prepared = _resolve_decode_requests_in_block(indexed)
                # 3. Decode field batches and scatter their results back into sample order.
                decoded = _decode_iter(
                    prepared=prepared,
                    decoders=decoders,
                    executor=executor,
                )
                for target, sample in zip(fetched.targets, decoded, strict=True):
                    _raise_if_incomplete(sample, target, required)
                    yield sample

            samples = _pipeline_blocks(blocks, fetch=fetch_block, process=process)
            yield from _count_yields(_replay(samples, emit_order))

    def _worker_order(self) -> tuple[np.ndarray, np.ndarray]:
        """Return this worker's shard of the epoch's `(indices, block_bounds)`, in fetch order."""
        indices, block_bounds = self._shuffle_strategy.epoch_order(
            self._sample_index,
            fetch_block_size=self._fetch_block_size,
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
