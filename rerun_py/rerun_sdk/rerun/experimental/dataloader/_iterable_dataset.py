"""IterableDataset backed by a catalog server."""

from __future__ import annotations

import time
import warnings
from typing import TYPE_CHECKING, Any

import numpy as np
import torch
import torch.utils.data

from rerun._tracing import set_current_span_attributes, tracing_scope, with_tracing

from ._config import TimeAlignedSampling
from ._metrics import DataloaderMetrics
from ._sample_index import SampleIndex
from ._shuffle import SampleShuffle, ShuffleStrategy
from ._utils import (
    FetchedBlock,
    FetchMetricsAccumulator,
    _build_query_plans,
    _build_targets,
    _decode_iter,
    _decode_pool,
    _discover_prior_keyframes,
    _fetch_queries_parallel,
    _index_fetched_block,
    _locate_samples,
    _pipeline_blocks,
    _record_query_metrics,
    _resolve_decode_requests_in_block,
    _resolve_decode_threads,
    _validate_source_schema,
    _warn_if_fork_unsafe,
    _WorkerConnection,
)
from .decoders._base import DecodedSample

if TYPE_CHECKING:
    from collections.abc import Generator, Iterator

    from rerun.experimental.spans import Curation

    from ._config import DataSource, Field, FixedRateSampling
    from ._metrics import _WorkerMetricsPublisher
    from ._shuffle import _WorkOrder


_DEFAULT_MAX_CONSECUTIVE_SKIPPED_SAMPLES = 1000


def _skip_incomplete(
    samples: Generator[DecodedSample, None, None],
    *,
    max_consecutive_skipped_samples: int | None = _DEFAULT_MAX_CONSECUTIVE_SKIPPED_SAMPLES,
    metrics: _WorkerMetricsPublisher | None = None,
) -> Generator[DecodedSample, None, None]:
    """Drop emitted samples with missing fields up to the configured limit."""
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
            if metrics is not None:
                metrics.record_skipped_samples()
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


def _live_ddp_topology() -> tuple[int, int] | None:
    """`(rank, world_size)` of this process's default process group, or `None` outside DDP."""
    if torch.distributed.is_available() and torch.distributed.is_initialized():
        return torch.distributed.get_rank(), torch.distributed.get_world_size()
    return None


class RerunIterableDataset(torch.utils.data.IterableDataset[DecodedSample]):
    """
    Iterable dataset backed by a catalog server.

    Fetches samples in blocks and yields them individually. The `DataLoader`
    controls the training batch size independently.

    The index list is partitioned across DDP ranks and DataLoader workers
    internally. With shuffling enabled (default), the sample order is permuted
    once per epoch before partitioning; call `set_epoch` to re-seed between
    epochs. When a finite dataset is consumed to exhaustion under DDP,
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
        `None` uses ordinary integer sampling.
        [`TimeAlignedSampling`][rerun.experimental.dataloader.TimeAlignedSampling] declares aligned integer fields.
        Use [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling]
        for timestamp and duration timelines.
    fetch_block_size
        Number of samples to fetch per block. Larger values
        amortize network overhead but use more memory. Defaults to 128.
    shuffle_strategy
        The [`ShuffleStrategy`][rerun.experimental.dataloader.ShuffleStrategy]
        that determines the order samples are fetched in, and — for
        [`BlockShuffle`][rerun.experimental.dataloader.BlockShuffle] — the
        optional post-decode buffer samples are emitted through. Defaults to
        [`SampleShuffle`][rerun.experimental.dataloader.SampleShuffle]; pass
        [`NoShuffle`][rerun.experimental.dataloader.NoShuffle] for natural order.
    seed
        Base shuffle seed. The epoch selects a reproducible order using `seed + epoch`.
    decode_threads
        Fields to decode concurrently within each `DataLoader` worker.
    max_consecutive_skipped_samples
        Maximum number of consecutive incomplete samples to skip in each
        iterator, independently for every rank and `DataLoader` worker. A valid
        sample resets the count. The next missing sample raises with total and
        per-field counts. Defaults to 1000; pass `None` to apply no limit.

    """

    def __init__(
        self,
        source: DataSource,
        index: str,
        fields: dict[str, Field],
        *,
        timeline_sampling: TimeAlignedSampling | FixedRateSampling | None = None,
        fetch_block_size: int = 128,
        shuffle_strategy: ShuffleStrategy | None = None,
        seed: int = 0,
        decode_threads: int | None = None,
        max_consecutive_skipped_samples: int | None = _DEFAULT_MAX_CONSECUTIVE_SKIPPED_SAMPLES,
    ) -> None:
        super().__init__()

        _warn_if_fork_unsafe(stacklevel=3)
        if type(seed) is not int or seed < 0:
            raise ValueError("seed must be a non-negative integer")
        if type(fetch_block_size) is not int or fetch_block_size <= 0:
            raise ValueError("fetch_block_size must be a positive integer")
        if max_consecutive_skipped_samples is not None and max_consecutive_skipped_samples < 0:
            raise ValueError(
                f"max_consecutive_skipped_samples must be non-negative, got {max_consecutive_skipped_samples}"
            )

        self._fields = fields
        self._timeline_sampling = timeline_sampling
        self._index = index
        self._fetch_block_size = fetch_block_size
        self._decode_threads = _resolve_decode_threads(decode_threads, fields)
        self._max_consecutive_skipped_samples = max_consecutive_skipped_samples

        self._shuffle_strategy = shuffle_strategy if shuffle_strategy is not None else SampleShuffle()
        self._shuffle_buffer = self._shuffle_strategy.emission_buffer()
        self._epoch = 0
        self._seed = seed
        self._parent_ddp_topology: tuple[int, int] | None = None

        _validate_source_schema(
            source,
            index,
            fields,
            expected_index_type=None if source.curation is None else source.curation.index_type,
        )
        self._sample_index = SampleIndex.build(
            source,
            index,
            self._fields,
            timeline_sampling=timeline_sampling,
        )
        self._metrics: DataloaderMetrics | None = DataloaderMetrics()
        self._metrics_publisher = self._metrics._worker_publisher()

        self._connection = _WorkerConnection.from_source(source, fields)

    @classmethod
    def from_curation(
        cls,
        curation: Curation,
        source: DataSource,
        fields: dict[str, Field],
        *,
        timeline_sampling: TimeAlignedSampling | FixedRateSampling | None = None,
        fetch_block_size: int = 128,
        shuffle_strategy: ShuffleStrategy | None = None,
        seed: int = 0,
        decode_threads: int | None = None,
        max_consecutive_skipped_samples: int | None = _DEFAULT_MAX_CONSECUTIVE_SKIPPED_SAMPLES,
    ) -> RerunIterableDataset:
        """
        Sample selected intervals with deterministic shuffling and rank/worker assignments.

        Sampling determines candidate anchors without reading source fields.
        Field windows may extend beyond selected intervals. Fields and video keyframes
        are resolved at runtime; missing samples are skipped
        after shuffling without replacement. Reproducing assignments requires the same
        source contents, settings, epoch, and rank/worker counts.
        """
        return cls(
            source._with_curation(curation),
            curation.index_name,
            fields,
            timeline_sampling=timeline_sampling,
            fetch_block_size=fetch_block_size,
            shuffle_strategy=shuffle_strategy,
            seed=seed,
            decode_threads=decode_threads,
            max_consecutive_skipped_samples=max_consecutive_skipped_samples,
        )

    @property
    def metrics(self) -> DataloaderMetrics:
        """Cumulative fetch and decode metrics aggregated across this dataset's workers."""
        if self._metrics is None:
            raise RuntimeError("Dataloader metrics are only available from the process that created the dataset")
        return self._metrics

    @property
    def sample_index(self) -> SampleIndex:
        """The underlying [`SampleIndex`][rerun.experimental.dataloader.SampleIndex]."""
        return self._sample_index

    def __len__(self) -> int:
        """Candidate count across all workers, before missing samples are skipped."""
        return self._sample_index.total_samples

    def set_epoch(self, epoch: int) -> None:
        """Set the epoch for shuffling (like `DistributedSampler.set_epoch`)."""
        self._epoch = epoch

    def __getstate__(self) -> dict[str, Any]:
        """Carry the parent's DDP rank into spawned workers, which have no process group of their own."""
        state = self.__dict__.copy()
        state["_parent_ddp_topology"] = _live_ddp_topology() or self._parent_ddp_topology
        state["_metrics"] = None
        return state

    def _ddp_topology(self) -> tuple[int, int]:
        """`(rank, world_size)` to shard by: this process's group, else the one captured when pickled."""
        return _live_ddp_topology() or self._parent_ddp_topology or (0, 1)

    def __iter__(self) -> Iterator[DecodedSample]:
        """
        Yield individual samples as they are decoded.

        The arrow fetch for block N+1 runs on a background thread while
        block N is being decoded, so samples stream out during decode.
        When the strategy defines an emission buffer, decoded samples pass
        through that buffer before being yielded.
        """
        with (
            tracing_scope("RerunIterableDataset.__iter__"),
            _decode_pool(self._decode_threads, len(self._fields)) as executor,
        ):
            work_order = self._worker_order()

            set_current_span_attributes({
                "rerun.dataloader.iter.num_candidates": work_order.num_samples,
                "rerun.dataloader.iter.num_samples_yielded": 0,
                "rerun.dataloader.shuffle_strategy": self._shuffle_strategy.RECIPE_TAG,
            })
            if work_order.num_samples == 0:
                return

            blocks = work_order.fetch_blocks(fetch_block_size=self._fetch_block_size)
            view, decoders = self._connection.ensure()

            @with_tracing("RerunDataset._fetch_block")
            def fetch_block(block: np.ndarray) -> FetchedBlock:
                fetch_metrics = FetchMetricsAccumulator()
                located = _locate_samples(
                    block,
                    sample_index=self._sample_index,
                    num_fields=len(self._fields),
                )
                with _record_query_metrics(fetch_metrics):
                    keyframes = (
                        {}
                        if isinstance(self._timeline_sampling, TimeAlignedSampling)
                        else _discover_prior_keyframes(
                            view=view,
                            index=self._index,
                            fields=self._fields,
                            located=located,
                            sample_index=self._sample_index,
                        )
                    )
                    targets = _build_targets(
                        located,
                        keyframes,
                        fields=self._fields,
                        sample_index=self._sample_index,
                        timeline_sampling=self._timeline_sampling,
                    )
                    query_plans = _build_query_plans(
                        targets,
                        self._fields,
                        sample_index=self._sample_index,
                        timeline_sampling=self._timeline_sampling,
                        index=self._index,
                    )
                    fetched_groups = _fetch_queries_parallel(query_plans, view=view, index=self._index)
                return FetchedBlock(
                    targets=targets,
                    fetched_groups=fetched_groups,
                    fetch_direct_original_ranges=fetch_metrics.direct_original_ranges,
                    fetch_direct_merged_ranges=fetch_metrics.direct_merged_ranges,
                    fetch_grpc_requests=fetch_metrics.grpc_requests,
                    delivered_bytes=fetch_metrics.delivered_bytes,
                )

            def process(fetched: FetchedBlock) -> Iterator[DecodedSample]:
                preparation_started_ns = time.perf_counter_ns()
                # 1. Group each fetched table into contiguous, index-ordered segment spans.
                indexed = _index_fetched_block(fetched, self._index)
                # 2. Map each requested timeline range to physical Arrow rows.
                prepared = _resolve_decode_requests_in_block(indexed)
                preparation_elapsed_ns = time.perf_counter_ns() - preparation_started_ns
                # 3. Decode field batches and scatter their results back into sample order.
                return _decode_iter(
                    prepared=prepared,
                    decoders=decoders,
                    executor=executor,
                    metrics=self._metrics_publisher,
                    preparation_elapsed_ns=preparation_elapsed_ns,
                )

            samples = _pipeline_blocks(blocks, fetch=fetch_block, process=process, metrics=self._metrics_publisher)
            if self._shuffle_buffer is not None:
                rank, _ = self._ddp_topology()
                worker_info = torch.utils.data.get_worker_info()
                worker_id = worker_info.id if worker_info is not None else 0
                rng = np.random.default_rng([self._seed + self._epoch, rank, worker_id])
                samples = self._shuffle_buffer.shuffle(samples, rng=rng)
            samples = _skip_incomplete(
                samples,
                max_consecutive_skipped_samples=self._max_consecutive_skipped_samples,
                metrics=self._metrics_publisher,
            )
            yield from _count_yields(samples)

    def _worker_order(self) -> _WorkOrder:
        """Return this worker's lazily materialized shard of the epoch order."""
        with tracing_scope("RerunIterableDataset._worker_order.epoch_order"):
            work_order = self._shuffle_strategy._work_order(
                self._sample_index,
                fetch_block_size=self._fetch_block_size,
                seed=self._seed + self._epoch,
            )
            set_current_span_attributes({"rerun.dataloader.worker_order.num_samples": work_order.num_samples})

        # Partition across distributed ranks first (DDP), then across
        # DataLoader workers within this rank. Contiguous (not interleaved)
        # slices keep a worker on a small set of segments.
        rank, world_size = self._ddp_topology()
        if world_size > 1:
            with tracing_scope("RerunIterableDataset._worker_order.ddp_shard"):
                work_order = work_order.shard(rank=rank, world_size=world_size)

        worker_info = torch.utils.data.get_worker_info()
        if worker_info is not None:
            with tracing_scope("RerunIterableDataset._worker_order.worker_shard"):
                work_order = work_order.shard(rank=worker_info.id, world_size=worker_info.num_workers)

        with tracing_scope("RerunIterableDataset._worker_order.compact"):
            work_order = work_order.compact()
            set_current_span_attributes({"rerun.dataloader.worker_order.num_samples": work_order.num_samples})

        return work_order
