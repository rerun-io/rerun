from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, runtime_checkable

from ._streaming_reader import StreamingReader

if TYPE_CHECKING:
    from ._chunk_store import ChunkStore


@runtime_checkable
class IndexedReader(StreamingReader, Protocol):
    """
    Protocol for readers that can produce a fully materialized ChunkStore.

    Extends `StreamingReader`: every `IndexedReader` also supports
    `stream() -> LazyChunkStream` for pure-streaming processing.
    """

    def store(self) -> ChunkStore:
        """Return a fully materialized ChunkStore from this source."""
        ...
