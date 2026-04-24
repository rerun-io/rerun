"""Map-style Dataset backed by the Rerun Data Platform."""

from __future__ import annotations

from typing import TYPE_CHECKING

import torch
import torch.utils.data

from rerun._tracing import with_tracing

from ._sample_index import FixedRateSampling, SampleIndex
from ._utils import _decode_iter, _fetch_arrow, _WorkerConnection

if TYPE_CHECKING:
    from ._config import Column, DataSource


class RerunMapDataset(torch.utils.data.Dataset[dict[str, torch.Tensor]]):
    """
    Map-style dataset backed by the Rerun Data Platform.

    Supports random access by global index, making it compatible with
    PyTorch's sampler ecosystem (`DistributedSampler`, `WeightedRandomSampler`, `SubsetRandomSampler`, …).

    Shuffling and cross-worker partitioning are driven by the `DataLoader`'s sampler.

    For simple in-order streaming with internal shuffling, use [`RerunIterableDataset`][rerun.experimental.dataloader.RerunIterableDataset] instead.

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

    Examples
    --------
    ```python
    dataset = RerunMapDataset(
        source,
        "frame_nr",
        {"image": Column("/camera:Image:blob", decode=ImageDecoder())},
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
        columns: dict[str, Column],
        *,
        timeline_sampling: FixedRateSampling | None = None,
    ) -> None:
        super().__init__()

        self._columns = columns
        self._index = index

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

    def __getitem__(self, idx: int) -> dict[str, torch.Tensor]:  # ty: ignore[invalid-method-override]
        """Fetch a single sample by global index (one server query)."""
        return self.__getitems__([idx])[0]

    @with_tracing("RerunMapDataset.__getitems__")
    def __getitems__(self, indices: list[int]) -> list[dict[str, torch.Tensor]]:
        """
        Fetch multiple samples by global index in a single server query.

        PyTorch's `DataLoader` calls this automatically when available,
        so each training batch round-trips only once.
        """
        view, decoders = self._connection.ensure()
        targets, seg_tables = _fetch_arrow(
            view=view,
            index=self._index,
            columns=self._columns,
            decoders=decoders,
            sample_index=self._sample_index,
            indices=indices,
        )
        return list(
            _decode_iter(
                targets=targets,
                seg_tables=seg_tables,
                index=self._index,
                columns=self._columns,
                decoders=decoders,
            ),
        )
