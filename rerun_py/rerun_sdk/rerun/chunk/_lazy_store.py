from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import Schema
    from rerun.experimental._chunk_index import ChunkIndex
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
    [`ChunkStore`][rerun.chunk.ChunkStore], call `lazy.stream().collect()`.

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

    def _chunk_index(self) -> ChunkIndex:
        """
        The store's chunk index (its raw RRD manifest), one row per chunk.

        Private and experimental. The manifest is already in memory, so this loads no
        chunk data.
        """
        from rerun.experimental._chunk_index import ChunkIndex

        store_id, batch = self._internal._chunk_index()
        return ChunkIndex(store_id, batch)

    def _optimized_stream(
        self,
        *,
        chunk_max_bytes: int | None = None,
        chunk_max_rows: int | None = None,
        chunk_max_rows_if_unsorted: int | None = None,
        target_timeline: str | None = None,
    ) -> LazyChunkStream:
        """
        Return a lazy stream of vertically optimized (merged/split) chunks.

        Private and experimental. The only optimization is vertical: merge and split
        chunks toward `chunk_max_bytes`, with `chunk_max_rows` as a row guard (`0`
        disables a limit). `chunk_max_rows_if_unsorted` replaces the row guard for
        outputs with at least one time-unsorted timeline. It is video-unaware and can
        undo GoP alignment. Defaults are the object-store profile. `target_timeline`
        orders the merge sweep by time; `None` means file order.
        """
        from ._lazy_chunk_stream import LazyChunkStream

        return LazyChunkStream(
            self._internal._optimized_stream(
                chunk_max_bytes=chunk_max_bytes,
                chunk_max_rows=chunk_max_rows,
                chunk_max_rows_if_unsorted=chunk_max_rows_if_unsorted,
                target_timeline=target_timeline,
            )
        )

    @property
    def _chunks_loaded(self) -> int:
        """
        Monotonic count of chunks physically loaded from this store since it was opened.

        For test purposes.
        """
        return self._internal._chunks_loaded

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
