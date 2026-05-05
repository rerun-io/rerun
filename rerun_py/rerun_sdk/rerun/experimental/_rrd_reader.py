from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import RrdReaderInternal

from ._lazy_chunk_stream import LazyChunkStream
from ._lazy_store import LazyStore

if TYPE_CHECKING:
    from pathlib import Path


class RrdReader:
    """
    Read chunks from an RRD file (streaming, sequential).

    Currently, the first Recording store is streamed. Blueprint stores and subsequent recording stores are ignored
    """

    # TODO(RR-4263): we eventually need to address the above limitation and provide better control to the user.

    _internal: RrdReaderInternal

    def __init__(self, path: str | Path) -> None:
        self._internal = RrdReaderInternal(str(path))

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in the RRD file."""
        return LazyChunkStream(self._internal.stream())

    def store(self) -> LazyStore:
        """
        Open the RRD as a [`LazyStore`][rerun.experimental.LazyStore].

        Reads the manifest immediately; chunk data is loaded on demand.
        Legacy RRDs without a footer/manifest are not supported here — use
        `RrdReader(...).stream().collect()` for those.
        """
        return LazyStore(self._internal.store())

    @property
    def application_id(self) -> str | None:
        """Application ID from the RRD's StoreInfo, if present."""
        return self._internal.application_id

    @property
    def recording_id(self) -> str | None:
        """Recording ID from the RRD's StoreInfo, if present."""
        return self._internal.recording_id

    @property
    def path(self) -> Path:
        """The file path of the RRD file."""
        return self._internal.path

    def __repr__(self) -> str:
        return f"RrdReader({self._internal.path})"
