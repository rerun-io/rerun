from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, kw_only=True)
class OptimizationSettings:
    """
    Settings for optimizing a ChunkStore via `LazyChunkStream.collect(optimize=...)`.

    Defaults mirror those of the `rerun rrd optimize` CLI. `None` on a threshold
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
    If `True` (default), video stream chunks are rebatched to align with GoP
    (keyframe) boundaries after normal compaction.

    GoP rebatching never splits a GoP across chunks, so streams with long keyframe
    intervals can produce chunks much larger than `max_bytes`.
    """

    split_size_ratio: float | None = None
    """
    If set, split chunks so no two archetype groups sharing a chunk differ in
    byte size by more than this factor. Values should be `>= 1`; at `1.0`,
    every archetype is forced into its own chunk.

    This keeps large columns (images, videos, blobs) out of the same chunk as
    small columns (scalars, transforms, text), so the viewer can fetch just the
    small columns without dragging along the large payload. Components belonging
    to the same archetype are always kept together.

    A good starting value is `10.0`. If `None` (default), no splitting is
    performed.
    """
