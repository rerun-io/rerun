from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import Schema
    from rerun_bindings import LazyStoreInternal

    from ._lazy_chunk_stream import LazyChunkStream


class LazyStore:
    """
    Index-based, on-demand chunk store.

    The manifest is held in memory (so `schema()`, `summary()`, and `__len__`
    work without loading any chunks), but chunk data is loaded only when
    requested.

    Example:
        lazy = RrdReader("recording.rrd").store()

    Use `stream()` to process chunks through the lazy pipeline, or `write_rrd()`
    to persist to disk. To fully materialize into a
    [`ChunkStore`][rerun.experimental.ChunkStore], call `lazy.stream().collect()`.

    """

    _internal: LazyStoreInternal

    def __init__(self, internal: LazyStoreInternal) -> None:
        self._internal = internal

    def schema(self) -> Schema:
        """The schema describing all columns in this store, derived from the manifest."""
        from rerun.catalog import Schema

        return Schema(self._internal.schema())

    def summary(self) -> str:
        """
        Compact, deterministic summary of every chunk in the store.

        Built from the manifest; no chunk data is loaded. Each line describes one chunk:

            {entity_path}  rows={n}  static={True|False}  timelines=[…]  cols=[…]

        Useful for snapshot testing.
        """
        return self._internal.summary()

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in this store."""
        from ._lazy_chunk_stream import LazyChunkStream

        return LazyChunkStream(self._internal.stream())

    def write_rrd(
        self,
        path: str | Path,
        *,
        application_id: str,
        recording_id: str,
    ) -> None:
        """
        Write all chunks to an RRD file.

        The caller must provide application_id and recording_id explicitly.
        """
        self.stream().write_rrd(
            path,
            application_id=application_id,
            recording_id=recording_id,
        )

    def __len__(self) -> int:
        """Return the number of chunks described by the manifest."""
        return self._internal.num_chunks()

    def __repr__(self) -> str:
        return f"LazyStore({len(self)} chunks)"
