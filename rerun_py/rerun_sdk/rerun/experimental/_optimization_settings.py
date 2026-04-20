from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, kw_only=True)
class OptimizationSettings:
    """
    Settings for optimizing a ChunkStore via `LazyChunkStream.collect(optimize=...)`.

    Defaults mirror those of the ``rerun rrd compact`` CLI. ``None`` on a threshold
    field means using the default internal value.
    """

    max_bytes: int | None = None
    """Chunk size threshold in bytes. ``None`` means use the default."""

    max_rows: int | None = None
    """Maximum rows per sorted chunk. ``None`` means use the default."""

    max_rows_if_unsorted: int | None = None
    """Maximum rows per unsorted chunk. ``None`` means use the default."""

    extra_passes: int = 50
    """Number of extra convergence passes run after the initial insert."""

    gop_batching: bool = True
    """
    If ``True`` (default), video stream chunks are rebatched to align with GoP
    (keyframe) boundaries after normal compaction.

    GoP rebatching never splits a GoP across chunks, so streams with long keyframe
    intervals can produce chunks much larger than ``max_bytes``.
    """
