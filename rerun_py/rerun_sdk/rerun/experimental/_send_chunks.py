from __future__ import annotations

from typing import TYPE_CHECKING

import rerun_bindings as bindings

from ._chunk import Chunk
from ._chunk_store import ChunkStore
from ._lazy_chunk_stream import LazyChunkStream
from ._lazy_store import LazyStore

if TYPE_CHECKING:
    from collections.abc import Iterable

    from rerun.recording_stream import RecordingStream
    from rerun_bindings import ChunkInternal


def _unwrap(c: object) -> ChunkInternal:
    """Validate-then-unwrap a single chunk for the bindings iterable arm."""
    if not isinstance(c, Chunk):
        raise TypeError(
            f"send_chunks expects Chunk objects in the iterable, got {type(c).__name__!r}",
        )
    return c._internal


def send_chunks(
    chunks: Chunk | LazyChunkStream | LazyStore | ChunkStore | Iterable[Chunk],
    *,
    recording: RecordingStream | None = None,
) -> None:
    """
    Send chunks to a recording stream. Blocks until every chunk has been queued.

    !!! note
        For a `LazyChunkStream` and `LazyStore` inputs, this call triggers execution
        and/or loading and will block for the duration of this process.

    Parameters
    ----------
    chunks:
        One of:

        - A single [`Chunk`][rerun.experimental.Chunk].
        - A [`LazyChunkStream`][rerun.experimental.LazyChunkStream] — consume
          the stream and forward all chunks to the recording stream.
        - A [`LazyStore`][rerun.experimental.LazyStore] — send all chunks to the
          recording stream. This triggers loading all chunks from the source.
        - A [`ChunkStore`][rerun.experimental.ChunkStore] — send all chunks to
          the recording stream (fast since all chunks are already loaded).
        - Any iterable of `Chunk` objects.

        Source store identity (`application_id`, `recording_id`) is **not**
        preserved: chunks adopt the destination recording's identity.
    recording:
        Recording stream to send into. Defaults to the current active recording.

    """
    native = recording.to_native() if recording is not None else None

    match chunks:
        case LazyStore() | ChunkStore():
            chunks.stream()._internal.send_to_recording(native)
        case LazyChunkStream():
            chunks._internal.send_to_recording(native)
        case Chunk():
            bindings.send_chunks(chunks._internal, recording=native)
        case _:  # Iterable[Chunk]
            bindings.send_chunks((_unwrap(c) for c in chunks), recording=native)
