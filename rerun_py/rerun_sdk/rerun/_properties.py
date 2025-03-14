from __future__ import annotations
from typing import Optional, TypedDict

import rerun_bindings as bindings

from .recording_stream import RecordingStream


class RecordingProperties(TypedDict, total=False):
    name: Optional[str]
    started: Optional[int]


def set_properties(properties: RecordingProperties, recording: RecordingStream | None = None) -> None:
    """
    Set the properties of the recording.

    These are builtin properties of the Rerun Viewer.

    Parameters
    ----------
    properties : RecordingProperties
        The name of the recording.

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

    """

    bindings.set_name(name, recording=recording.to_native() if recording is not None else None)
