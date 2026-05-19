from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import RrdReaderInternal

from ._chunk_store import ChunkStore
from ._lazy_chunk_stream import LazyChunkStream

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
        # TODO(RR-4321): this should probably be self.store().stream() instead, when `ChunkStore` is lazily loaded
        return LazyChunkStream(self._internal.stream())

    def store(self) -> ChunkStore:
        """Load the entire RRD into a fully materialized ChunkStore."""
        return ChunkStore(self._internal.store())

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
