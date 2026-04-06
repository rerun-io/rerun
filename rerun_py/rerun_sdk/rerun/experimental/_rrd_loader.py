from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import RrdLoaderInternal

from ._lazy_chunk_stream import LazyChunkStream

if TYPE_CHECKING:
    from pathlib import Path


class RrdLoader:
    """
    Load chunks from an RRD file (streaming, sequential).

    Currently, the first Recording store is streamed. Blueprint stores and subsequent recording stores are ignored
    """

    # TODO(RR-4263): we eventually need to address the above limitation and provide better control to the user.

    _internal: RrdLoaderInternal

    def __init__(self, path: str | Path) -> None:
        self._internal = RrdLoaderInternal(str(path))

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in the RRD file."""
        return LazyChunkStream(self._internal.stream())

    @property
    def application_id(self) -> str | None:
        """Application ID from the RRD's StoreInfo, if present."""
        return self._internal.application_id

    @property
    def recording_id(self) -> str | None:
        """Recording ID from the RRD's StoreInfo, if present."""
        return self._internal.recording_id

    def __repr__(self) -> str:
        return f"RrdLoader({self._internal.application_id!r})"
