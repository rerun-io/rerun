from __future__ import annotations

from typing import TYPE_CHECKING, Literal

from rerun_bindings import McapLoaderInternal

from ._lazy_chunk_stream import LazyChunkStream

if TYPE_CHECKING:
    from collections.abc import Sequence
    from pathlib import Path


class McapLoader:
    """Load chunks from an MCAP file."""

    _internal: McapLoaderInternal

    # TODO(ab): this API is a reflection of the current state of the MCAP loader and mirrors `rerun mcap convert`. It's
    #  far from perfect and should be improved as the MCAP loader stabilizes.
    def __init__(
        self,
        path: str | Path,
        *,
        timeline_type: Literal["timestamp", "duration"] = "timestamp",
        timestamp_offset_ns: int | None = None,
        decoders: Sequence[str] | None = None,
    ) -> None:
        self._internal = McapLoaderInternal(
            str(path),
            timeline_type=timeline_type,
            timestamp_offset_ns=timestamp_offset_ns,
            decoders=list(decoders) if decoders is not None else None,
        )

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in the MCAP file."""
        return LazyChunkStream(self._internal.stream())

    def __repr__(self) -> str:
        return f"McapLoader({self._internal.path!r})"

    @staticmethod
    def available_decoders() -> list[str]:
        """Return the list of all supported MCAP decoder identifiers."""
        return McapLoaderInternal.available_decoders()
