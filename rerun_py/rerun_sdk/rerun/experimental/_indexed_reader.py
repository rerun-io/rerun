from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, runtime_checkable

from ._streaming_reader import StreamingReader

if TYPE_CHECKING:
    from ._lazy_store import LazyStore


@runtime_checkable
class IndexedReader(StreamingReader, Protocol):
    """
    Protocol for readers backed by an index/manifest.

    Extends `StreamingReader`: every `IndexedReader` also supports
    `stream() -> LazyChunkStream` for pure-streaming processing.

    Indexed readers expose a [`LazyStore`][rerun.experimental.LazyStore] view
    over the source via `store()` — the manifest is read up-front; chunks load
    on demand. To fully materialize into a
    [`ChunkStore`][rerun.experimental.ChunkStore], call `stream().collect()`.
    """

    def store(self) -> LazyStore:
        """Return a [`LazyStore`][rerun.experimental.LazyStore] view of this source."""
        ...
