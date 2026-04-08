from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, runtime_checkable

from ._streaming_loader import StreamingLoader

if TYPE_CHECKING:
    from ._chunk_store import ChunkStore


@runtime_checkable
class IndexedLoader(StreamingLoader, Protocol):
    """
    Protocol for loaders that can produce a fully materialized ChunkStore.

    Extends ``StreamingLoader``: every ``IndexedLoader`` also supports
    ``stream() -> LazyChunkStream`` for pure-streaming processing.
    """

    def store(self) -> ChunkStore:
        """Return a fully materialized ChunkStore from this source."""
        ...
