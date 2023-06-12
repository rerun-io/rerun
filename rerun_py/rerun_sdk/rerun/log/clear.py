from __future__ import annotations

from rerun import bindings
from rerun.recording_stream import RecordingStream


def log_cleared(
    entity_path: str,
    *,
    recursive: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Indicate that an entity at a given path should no longer be displayed.

    If `recursive` is True this will also clear all sub-paths

    Parameters
    ----------
    entity_path:
        The path of the affected entity.

    recursive:
        Should this apply to all entity paths below `entity_path`?

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    """
    recording = RecordingStream.to_native(recording)
    bindings.log_cleared(entity_path, recursive, recording=recording)
