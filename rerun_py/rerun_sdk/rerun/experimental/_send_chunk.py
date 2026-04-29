from __future__ import annotations

from typing import TYPE_CHECKING

import rerun_bindings as bindings

if TYPE_CHECKING:
    from rerun.recording_stream import RecordingStream

    from ._chunk import Chunk


def send_chunk(
    chunk: Chunk,
    *,
    recording: RecordingStream | None = None,
) -> None:
    """
    Send a pre-built [`Chunk`][rerun.experimental.Chunk] to a recording stream.

    Parameters
    ----------
    chunk:
        The chunk to send.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording.

    """
    bindings.send_chunk(
        chunk=chunk._internal,
        recording=recording.to_native() if recording is not None else None,
    )
