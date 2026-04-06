from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, runtime_checkable

if TYPE_CHECKING:
    from ._lazy_chunk_stream import LazyChunkStream


@runtime_checkable
class StreamingLoader(Protocol):
    """
    Protocol for loaders that produce a sequential stream of chunks.

    All loaders provide ``stream() -> LazyChunkStream``. Loaders for indexable
    formats will additionally satisfy ``IndexedLoader`` (future) and provide
    ``store() -> ChunkStore``.
    """

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks from this source."""
        ...
