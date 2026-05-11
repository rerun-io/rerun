from __future__ import annotations

from dataclasses import dataclass
from typing import ClassVar


@dataclass(frozen=True, kw_only=True)
class OptimizationProfile:
    """
    Named optimization profile passed to `LazyChunkStream.collect(optimize=...)`.

    Two presets:

    - `OptimizationProfile.LIVE`: small chunks tuned for the live Viewer workflow.
    - `OptimizationProfile.OBJECT_STORE`: large chunks tuned for object-store-backed
      query and streaming (e.g. the Rerun Data Platform).

    The presets are *fully concrete*: every field has a value. Custom profiles
    built by calling `OptimizationProfile(...)` directly may pass `None` on the
    threshold fields to fall back to the SDK's internal default
    (`OptimizationProfile.LIVE`'s thresholds).
    """

    LIVE: ClassVar[OptimizationProfile]
    """
    Optimized for the live Viewer workflow: small chunks for low-latency
    rendering and fine-grained time-panel precision.
    """

    OBJECT_STORE: ClassVar[OptimizationProfile]
    """
    Optimized for object-store-backed storage (e.g. the Rerun Data Platform):
    larger chunks tuned for query throughput and streaming over the network.
    """

    max_bytes: int | None = None
    """Chunk size threshold in bytes. ``None`` means use `LIVE`'s default."""

    max_rows: int | None = None
    """Maximum rows per sorted chunk. ``None`` means use `LIVE`'s default."""

    max_rows_if_unsorted: int | None = None
    """Maximum rows per unsorted chunk. ``None`` means use `LIVE`'s default."""

    extra_passes: int = 50
    """Number of extra convergence passes run after the initial insert."""

    gop_batching: bool = True
    """
    If `True` (default), video stream chunks are rebatched to align with GoP
    (keyframe) boundaries after normal compaction.

    GoP rebatching never splits a GoP across chunks, so streams with long
    keyframe intervals can produce chunks much larger than `max_bytes`.
    """

    split_size_ratio: float | None = None
    """
    If set, split chunks so no two archetype groups sharing a chunk differ in
    byte size by more than this factor. Values should be `>= 1`; at `1.0`,
    every archetype is forced into its own chunk.

    This keeps large columns (images, videos, blobs) out of the same chunk as
    small columns (scalars, transforms, text), so the viewer can fetch just
    the small columns without dragging along the large payload. Components
    belonging to the same archetype are always kept together.

    A good starting value is `10.0`. If `None` (default), no splitting is
    performed.
    """


OptimizationProfile.LIVE = OptimizationProfile(
    max_bytes=12 * 8 * 4096,
    max_rows=4096,
    max_rows_if_unsorted=1024,
    extra_passes=50,
    gop_batching=True,
    split_size_ratio=None,
)

OptimizationProfile.OBJECT_STORE = OptimizationProfile(
    max_bytes=2 * 1024 * 1024,
    max_rows=65_536,
    max_rows_if_unsorted=8_192,
    extra_passes=50,
    gop_batching=True,
    split_size_ratio=None,
)
