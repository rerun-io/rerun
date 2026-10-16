from __future__ import annotations

from typing import TYPE_CHECKING

from rerun._tracing import with_tracing

if TYPE_CHECKING:
    from rerun_bindings import LayerInternal

    from . import DatasetEntry


class Layer:
    """
    A lightweight reference to one named layer across a dataset's segments.

    Obtain a handle with [`DatasetEntry.layer()`][rerun.catalog.DatasetEntry.layer].
    The handle retains its originating dataset entry, but does not cache layer metadata.
    """

    __slots__ = ("_internal",)

    def __init__(self, inner: LayerInternal) -> None:
        self._internal = inner

    def __repr__(self) -> str:
        return repr(self._internal)

    def __str__(self) -> str:
        return str(self._internal)

    @property
    def name(self) -> str:
        """The layer name within its dataset."""
        return self._internal.name

    @property
    def dataset(self) -> DatasetEntry:
        """The dataset entry that created this layer handle."""
        from ._entry import DatasetEntry

        return DatasetEntry(self._internal.dataset)

    @with_tracing("Layer.segment_ids")
    def segment_ids(self) -> list[str]:
        """
        Fetch the IDs of segments that currently contain this layer.

        Returns an empty list if no segments contain the layer.
        Raises [`NotFoundError`][rerun.catalog.NotFoundError] if the dataset does not exist.
        """
        return self._internal.segment_ids()
