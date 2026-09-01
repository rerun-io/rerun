"""
Experimental features for Rerun.

These features are not yet stable and may change in future releases without
going through the normal deprecation cycle.

The stable chunk API and readers (`RrdReader`, `McapReader`, …) now live in `rerun.chunk`.
The old `rerun.experimental` names still work for one release but warn on use.
"""

from __future__ import annotations

from typing import Any

from . import video as video
from ._hdf5_reader import DatasetInfo as DatasetInfo, Hdf5Reader as Hdf5Reader
from ._mp4_reader import Mp4Reader as Mp4Reader, Mp4TranscodeOptions as Mp4TranscodeOptions
from ._parquet_reader import ParquetReader as ParquetReader
from ._query_metrics import (
    MetricsCollector as MetricsCollector,
    QueryMetrics as QueryMetrics,
    query_metrics as query_metrics,
)
from ._viewer_client import ViewerClient as ViewerClient

# TODO(RR-5534): remove this deprecation shim one release after the chunk API move ships.
# The names below moved to `rerun.chunk`; `send_chunks` moved to the top-level `rerun`.
# Forward them with a warning for one release.
_MOVED_TO_CHUNK = frozenset({
    "Chunk",
    "ChunkStore",
    "LazyChunkStream",
    "LazyStore",
    "StoreEntry",
    "Lens",
    "DeriveLens",
    "MutateLens",
    "Selector",
    "IndexColumn",
    "OptimizationProfile",
    "RrdReader",
    "McapReader",
    "McapChannelInfo",
    "McapChunkInfo",
    "McapCompressionInfo",
    "McapInfo",
    "McapSchemaInfo",
    "StreamingReader",
    "IndexedReader",
})


def __getattr__(name: str) -> Any:
    """Forward moved names to their new home with a deprecation warning (see PEP 562)."""
    if name == "send_chunks":
        import warnings

        import rerun

        warnings.warn(
            "`rerun.experimental.send_chunks` moved to `rerun.send_chunks`. "
            "The `rerun.experimental` alias is deprecated and will be removed in a future release.",
            DeprecationWarning,
            stacklevel=2,
        )
        return rerun.send_chunks
    if name in _MOVED_TO_CHUNK:
        import warnings

        from .. import chunk

        warnings.warn(
            f"`rerun.experimental.{name}` moved to `rerun.chunk.{name}`. "
            f"The `rerun.experimental` alias is deprecated and will be removed in a future release.",
            DeprecationWarning,
            stacklevel=2,
        )
        return getattr(chunk, name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
