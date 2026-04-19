from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Sequence
    from pathlib import Path

    from rerun.catalog import Schema
    from rerun_bindings import ChunkStoreInternal

    from ._chunk import Chunk
    from ._lazy_chunk_stream import LazyChunkStream


class ChunkStore:
    """
    A chunk store.

    TODO(RR-4321): currently, this is fully materialized, in-memory.

    Obtain a ChunkStore from an IndexedReader, e.g.:

        store = RrdReader("recording.rrd").store()

    Use ``stream()`` to process chunks through the lazy pipeline, or
    ``write_rrd()`` to persist to disk.
    """

    _internal: ChunkStoreInternal

    def __init__(self, internal: ChunkStoreInternal) -> None:
        self._internal = internal

    @staticmethod
    def from_chunks(chunks: Sequence[Chunk]) -> ChunkStore:
        """Build a ChunkStore from a sequence of chunks."""
        from rerun_bindings import ChunkStoreInternal

        internals = [c._internal for c in chunks]
        return ChunkStore(ChunkStoreInternal.from_chunks(internals))

    def schema(self) -> Schema:
        """The schema describing all columns in this store."""
        from rerun.catalog import Schema

        return Schema(self._internal.schema())

    def summary(self) -> str:
        """
        Compact, deterministic summary of every chunk in the store.

        Each line describes one chunk:

            {entity_path}  rows={n}  bytes={…}  static={True|False}  timelines=[…]  cols=[…]

        Useful for snapshot testing.

        **Important**: For lazily-loaded stores, this forces loading all chunk data from disk.
        """
        return self._internal.summary()

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in this store."""
        from ._lazy_chunk_stream import LazyChunkStream

        return LazyChunkStream(self._internal.stream())

    def compact(self, *, max_bytes: int | None = None, gop_batching: bool = True) -> ChunkStore:
        """
        Return a new ChunkStore with chunks compacted for optimal storage.

        Parameters
        ----------
        max_bytes:
            Override the per-chunk byte ceiling used when merging chunks.
            If `None`, uses the store's default (~384 KiB).
        gop_batching:
            If `True` (default), video stream chunks are additionally rebatched
            to align with GoP (keyframe) boundaries after normal compaction.

            GoP rebatching never splits a GoP across chunks, so streams with long
            keyframe intervals (e.g. 10+ seconds between I-frames) can produce
            chunks much larger than `max_bytes`.

        """
        return ChunkStore(self._internal.compact(max_bytes=max_bytes, gop_batching=gop_batching))

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
        """Return the number of chunks in this store."""
        return self._internal.num_chunks()

    def __repr__(self) -> str:
        return f"ChunkStore({len(self)} chunks)"
