from __future__ import annotations

from typing import TYPE_CHECKING

from rerun._tracing import with_tracing

if TYPE_CHECKING:
    from rerun_bindings import SegmentInternal

    from . import DatasetEntry, UnregistrationHandle


class Segment:
    """
    A lightweight reference to one segment in a dataset.

    Obtain a handle with [`DatasetEntry.segment()`][rerun.catalog.DatasetEntry.segment].
    The handle retains its originating dataset entry, but does not cache segment metadata.
    """

    __slots__ = ("_internal",)

    def __init__(self, inner: SegmentInternal) -> None:
        self._internal = inner

    def __repr__(self) -> str:
        return repr(self._internal)

    def __str__(self) -> str:
        return str(self._internal)

    @property
    def id(self) -> str:
        """The segment ID within its dataset."""
        return self._internal.id

    @property
    def dataset(self) -> DatasetEntry:
        """The dataset entry that created this segment handle."""
        from ._entry import DatasetEntry

        return DatasetEntry(self._internal.dataset)

    @property
    def url(self) -> str:
        """The URL for this segment in the Rerun Viewer."""
        return self._internal.url

    @with_tracing("Segment.layers")
    def layers(self) -> set[str]:
        """
        Read the current layer names from the segment table.

        Raises [`NotFoundError`][rerun.catalog.NotFoundError] if the segment or dataset does not exist.
        """
        return self._internal.layers()

    @with_tracing("Segment.delete_layer")
    def delete_layer(self, name: str) -> UnregistrationHandle:
        """
        Unregister one layer from this segment and return a handle to track completion.

        Call `wait()` on the returned handle to wait for deletion.
        Other segments and layers are unchanged.
        A missing segment or layer is a no-op; a missing dataset raises
        [`NotFoundError`][rerun.catalog.NotFoundError].
        """
        from ._unregistration_handle import UnregistrationHandle

        return UnregistrationHandle(self._internal.delete_layer(name))
