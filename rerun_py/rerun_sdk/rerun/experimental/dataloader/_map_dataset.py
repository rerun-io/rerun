"""Map-style Dataset backed by a catalog server."""

from __future__ import annotations

from typing import TYPE_CHECKING

import torch.utils.data

from rerun._tracing import with_tracing

from ._sample_index import FixedRateSampling, SampleIndex
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
    _resolve_decode_requests_in_block,
    _resolve_decode_threads,
    _warn_if_fork_unsafe,
    _WorkerConnection,
)
from .decoders._base import DecodedSample

if TYPE_CHECKING:
    from ._config import DataSource, Field
    from .manifest._manifest import Manifest


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
        Required when `index` is a timestamp timeline; ignored for
        integer indices. Pass [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling] to sample on
        a fixed grid (e.g. 30 Hz).
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
        timeline_sampling: FixedRateSampling | None = None,
        decode_threads: int | None = None,
    ) -> None:
        super().__init__()

        _warn_if_fork_unsafe(stacklevel=3)

        self._fields = fields
        self._index = index
        self._decode_threads = _resolve_decode_threads(decode_threads, fields)
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
    ) -> RerunMapDataset:
        """
        Build a map-style dataset over a frozen [`Manifest`][rerun.experimental.dataloader.Manifest]'s validated samples.

        This is a **performance optimization only**: it reuses the manifest's validated sample set
        and frozen decode ranges, so there is no live scan and no per-batch keyframe lookup.

        !!! warning
            The manifest's recorded order is **not** respected here. Ordering and cross-worker
            sharding stay with the `DataLoader`'s sampler, as for any map-style dataset, so this
            cannot reproduce a manifest's run. For reproducible, resumable training use
            [`RerunIterableDataset.from_manifest`][rerun.experimental.dataloader.RerunIterableDataset.from_manifest].

        Parameters
        ----------
        manifest:
            The frozen manifest to read. Provides the validated sample set and decode ranges.
        source:
            The live catalog connection.
        fields:
            The decoders, keyed by field name (a manifest records only their spec, not the objects).
        decode_threads:
            Fields to decode concurrently within each `DataLoader` worker.

        Returns
        -------
        RerunMapDataset
            A dataset over the manifest's validated samples.

        """
        self = cls.__new__(cls)
        torch.utils.data.Dataset.__init__(self)
        _warn_if_fork_unsafe(stacklevel=3)
        meta = manifest.metadata
        self._fields = fields
        self._index = meta.index_name
        self._manifest = manifest
        self._decode_threads = _resolve_decode_threads(decode_threads, fields)
        # Query construction only needs the grid step / dtype, not the full sample space.
        self._sample_index = SampleIndex([], ns_per_sample=meta.ns_per_sample, ns_dtype=meta.ns_dtype)
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

    def __getitem__(self, idx: int) -> DecodedSample:  # ty: ignore[invalid-method-override]
        """Fetch a single sample by global index (one server query)."""
        return self.__getitems__([idx])[0]

    @with_tracing("RerunMapDataset.__getitems__")
    def __getitems__(self, indices: list[int]) -> list[DecodedSample]:
        """
        Fetch multiple samples by global index in a single server query.

        PyTorch's `DataLoader` calls this automatically when present, so
        each batch round-trips once.
        """
        view, decoders = self._connection.ensure()
        if self._manifest is not None:
            from .manifest._manifest_read import targets_from_rows

            rows = self._manifest.to_arrow().take(indices)
            targets = targets_from_rows(rows, fields=self._fields, sample_index=self._sample_index)
        else:
            located = _locate_samples(
                indices,
                sample_index=self._sample_index,
                num_fields=len(self._fields),
            )
            keyframes = _fetch_prior_keyframes(
                view=view,
                index=self._index,
                fields=self._fields,
                located=located,
                sample_index=self._sample_index,
            )
            targets = _build_targets(
                located,
                keyframes,
                fields=self._fields,
                sample_index=self._sample_index,
            )

        query_plans = _build_query_plans(targets, self._fields, sample_index=self._sample_index)
        fetched_groups = _fetch_queries_parallel(query_plans, view=view, index=self._index)
        fetched = FetchedBlock(targets=targets, fetched_groups=fetched_groups)
        with _decode_pool(self._decode_threads, len(self._fields)) as executor:
            # 1. Group each fetched table into contiguous, index-ordered segment spans.
            indexed = _index_fetched_block(fetched, self._index)
            # 2. Map each requested timeline range to physical Arrow rows.
            prepared = _resolve_decode_requests_in_block(indexed)
            # 3. Decode field batches and scatter their results back into sample order.
            return list(
                _decode_iter(
                    prepared=prepared,
                    decoders=decoders,
                    executor=executor,
                ),
            )
