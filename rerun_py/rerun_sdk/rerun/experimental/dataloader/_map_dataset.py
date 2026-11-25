"""Map-style Dataset backed by a catalog server."""

from __future__ import annotations

import time
from typing import TYPE_CHECKING

import torch.utils.data

from rerun._tracing import with_tracing

from ._config import TimeAlignedSampling
from ._metrics import DataloaderMetrics
from ._sample_index import SampleIndex
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
    _record_query_metrics,
    _resolve_decode_requests_in_block,
    _resolve_decode_threads,
    _validate_source_schema,
    _warn_if_fork_unsafe,
    _WorkerConnection,
)
from .decoders._base import DecodedSample

if TYPE_CHECKING:
    from rerun.experimental.spans import Curation

    from ._config import DataSource, Field, FixedRateSampling


class RerunMapDataset(torch.utils.data.Dataset[DecodedSample]):
    """
    Map-style dataset backed by a catalog server.

    Supports random access by global index, so it works with PyTorch's
    sampler ecosystem (`DistributedSampler`, `WeightedRandomSampler`,
    `SubsetRandomSampler`, ...). Shuffling and cross-worker partitioning
    are driven by the `DataLoader`'s sampler.

    For streaming iteration with internal shuffling, use
    [`RerunIterableDataset`][rerun.experimental.dataloader.RerunIterableDataset] instead.

    Parameters
    ----------
    source
        The dataset to read from (with optional segment filter).
    index
        Timeline column to use as the sample index (e.g. `"frame_nr"`).
    fields
        Sample fields, keyed by output name.
    timeline_sampling
        `None` uses ordinary integer sampling.
        [`TimeAlignedSampling`][rerun.experimental.dataloader.TimeAlignedSampling] declares aligned integer fields.
        Use [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling]
        for timestamp and duration timelines.
    decode_threads
        Fields to decode concurrently within each `DataLoader` worker.

    Examples
    --------
    ```python
    dataset = RerunMapDataset(
        source,
        "frame_nr",
        {"image": Field("/camera:Image:blob", decode=ImageDecoder())},
    )
    sampler = DistributedSampler(dataset)
    loader = DataLoader(dataset, batch_size=8, sampler=sampler, num_workers=4)
    for batch in loader:
        ...
    ```

    """

    def __init__(
        self,
        source: DataSource,
        index: str,
        fields: dict[str, Field],
        *,
        timeline_sampling: TimeAlignedSampling | FixedRateSampling | None = None,
        decode_threads: int | None = None,
    ) -> None:
        super().__init__()

        _warn_if_fork_unsafe(stacklevel=3)

        self._fields = fields
        self._timeline_sampling = timeline_sampling
        self._index = index
        self._decode_threads = _resolve_decode_threads(decode_threads, fields)

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
        decode_threads: int | None = None,
    ) -> RerunMapDataset:
        """
        Index candidates from selected intervals in canonical segment and timeline order.

        Field windows may extend beyond selected intervals. Overlapping selections
        produce one candidate per anchor. Missing values remain in returned sample
        dictionaries; the DataLoader's sampler controls ordering and worker ownership.
        Fields and keyframes are resolved at runtime within the source selection.
        """
        return cls(
            source._with_curation(curation),
            curation.index_name,
            fields,
            timeline_sampling=timeline_sampling,
            decode_threads=decode_threads,
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
        """Number of candidates, including candidates with missing field values."""
        return self._sample_index.total_samples

    def __getstate__(self) -> dict[str, object]:
        """Keep the parent-owned metrics reader out of spawned workers."""
        state = self.__dict__.copy()
        state["_metrics"] = None
        return state

    def __getitem__(self, idx: int) -> DecodedSample:  # ty: ignore[invalid-method-override]
        """Fetch a single sample by global index."""
        return self.__getitems__([idx])[0]

    @with_tracing("RerunMapDataset.__getitems__")
    def __getitems__(self, indices: list[int]) -> list[DecodedSample]:
        """
        Fetch multiple samples by global index, sharing queries where possible.

        PyTorch's `DataLoader` calls this once per batch.
        """
        view, decoders = self._connection.ensure()
        fetch_metrics = FetchMetricsAccumulator()
        fetch_started_ns = time.perf_counter_ns()
        with _record_query_metrics(fetch_metrics):
            located = _locate_samples(
                indices,
                sample_index=self._sample_index,
                num_fields=len(self._fields),
            )
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
        fetched = FetchedBlock(
            targets=targets,
            fetched_groups=fetched_groups,
            fetch_direct_original_ranges=fetch_metrics.direct_original_ranges,
            fetch_direct_merged_ranges=fetch_metrics.direct_merged_ranges,
            fetch_grpc_requests=fetch_metrics.grpc_requests,
            delivered_bytes=fetch_metrics.delivered_bytes,
        )
        fetch_elapsed_ns = time.perf_counter_ns() - fetch_started_ns
        self._metrics_publisher.record_fetch(
            stall_ns=fetch_elapsed_ns,
            samples_fetched=len(fetched.targets),
            direct_original_ranges=fetch_metrics.direct_original_ranges,
            direct_merged_ranges=fetch_metrics.direct_merged_ranges,
            grpc_requests=fetch_metrics.grpc_requests,
            delivered_bytes=fetch_metrics.delivered_bytes,
        )
        with _decode_pool(self._decode_threads, len(self._fields)) as executor:
            preparation_started_ns = time.perf_counter_ns()
            # 1. Group each fetched table into contiguous, index-ordered segment spans.
            indexed = _index_fetched_block(fetched, self._index)
            # 2. Map each requested timeline range to physical Arrow rows.
            prepared = _resolve_decode_requests_in_block(indexed)
            preparation_elapsed_ns = time.perf_counter_ns() - preparation_started_ns
            # 3. Decode field batches and scatter their results back into sample order.
            samples = list(
                _decode_iter(
                    prepared=prepared,
                    decoders=decoders,
                    executor=executor,
                    metrics=self._metrics_publisher,
                    preparation_elapsed_ns=preparation_elapsed_ns,
                ),
            )
        self._metrics_publisher.flush()
        return samples
