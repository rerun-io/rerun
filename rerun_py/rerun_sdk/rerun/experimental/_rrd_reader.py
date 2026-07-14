from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import RrdReaderInternal

from ._lazy_chunk_stream import LazyChunkStream
from ._lazy_store import LazyStore
from ._store_entry import StoreEntry

if TYPE_CHECKING:
    from pathlib import Path


class RrdReader:
    """
    Read chunks from an RRD file.

    Use `recordings()` or `blueprints()` to discover what stores exist in the file,
    then `stream()` or `store()` to access a specific one. When no store is
    specified, the first recording store is used.
    """

    _internal: RrdReaderInternal

    def __init__(self, path: str | Path) -> None:
        self._internal = RrdReaderInternal(str(path))

    def recordings(self) -> list[StoreEntry]:
        """List the recording entries in this RRD file."""
        return [StoreEntry(s) for s in self._internal.store_entries() if s.kind == "recording"]

    def blueprints(self) -> list[StoreEntry]:
        """List the blueprint entries in this RRD file."""
        return [StoreEntry(s) for s in self._internal.store_entries() if s.kind == "blueprint"]

    def stream(self, *, store: StoreEntry | None = None) -> LazyChunkStream:
        """
        Return a lazy stream over chunks from a store.

        Parameters
        ----------
        store:
            Which store to stream. If `None`, uses the first recording store.

        Raises
        ------
        ValueError
            If the specified store is not in this RRD file, or `None` was passed
            and the file contains no recording stores.

        """
        internal_store = store._internal if store is not None else None
        return LazyChunkStream(self._internal.stream(store=internal_store))

    def store(self, *, store: StoreEntry | None = None) -> LazyStore:
        """
        Open a specific store as a [`LazyStore`][rerun.experimental.LazyStore].

        Reads the manifest immediately; chunk data is loaded on demand.
        Legacy RRDs without a footer/manifest are not supported here — use
        `RrdReader(...).stream().collect()` for those.

        Parameters
        ----------
        store:
            Which store to load. If `None`, uses the first recording store.

        Raises
        ------
        ValueError
            If the specified store is not in this RRD file, or `None` was passed
            and the file contains no recording stores.

        """
        internal_store = store._internal if store is not None else None
        return LazyStore(self._internal.store(store=internal_store))

    @property
    def path(self) -> Path:
        """The file path of the RRD file."""
        return self._internal.path

    def __repr__(self) -> str:
        return f"RrdReader({self._internal.path})"
