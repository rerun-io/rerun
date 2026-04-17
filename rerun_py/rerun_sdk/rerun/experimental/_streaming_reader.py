from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, runtime_checkable

if TYPE_CHECKING:
    from ._lazy_chunk_stream import LazyChunkStream


@runtime_checkable
class StreamingReader(Protocol):
    """
    Protocol for readers that produce a sequential stream of chunks.

    All readers provide ``stream() -> LazyChunkStream``. Readers for indexable
    formats will additionally satisfy ``IndexedReader`` (future) and provide
    ``store() -> ChunkStore``.
    """

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks from this source."""
        ...
