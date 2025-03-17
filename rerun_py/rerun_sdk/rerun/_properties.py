from __future__ import annotations

import rerun_bindings as bindings

from .recording_stream import RecordingStream


def set_properties(properties: bindings.RecordingProperties, recording: RecordingStream | None = None) -> None:
    """
    Set the properties of the recording.

    These are builtin recording properties known to the Rerun viewer.

    Parameters
    ----------
    properties : RecordingProperties
        The properties of the recording.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.set_properties(properties, recording=recording.to_native() if recording is not None else None)


def set_name(name: str, recording: RecordingStream | None = None) -> None:
    """
    Set the name of the recording.

    This name is shown in the Rerun Viewer.

    Parameters
    ----------
    name : str
        The name of the recording.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.set_name(name, recording=recording.to_native() if recording is not None else None)
