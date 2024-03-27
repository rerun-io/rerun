"""Helper functions for directly working with recordings."""
from __future__ import annotations

import base64
import logging
import random
import string
from typing import Any

from rerun import bindings

from .recording_stream import RecordingStream


def memory_recording(recording: RecordingStream | None = None) -> MemoryRecording:
    """
    Streams all log-data to a memory buffer.

    This can be used to display the RRD to alternative formats such as html.
    See: [rerun.MemoryRecording.as_html][].

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    Returns
    -------
    MemoryRecording
        A memory recording object that can be used to read the data.

    """

    recording = RecordingStream.to_native(recording)
    return MemoryRecording(bindings.memory_recording(recording=recording))


class MemoryRecording:
    """A recording that stores data in memory."""

    def __init__(self, storage: bindings.PyMemorySinkStorage) -> None:
        self.storage = storage

    def reset_data(self) -> None:
        """Reset the data in the MemoryRecording."""
        self.storage.reset_data()

    def reset_blueprint(self, *, add_to_app_default_blueprint: bool = False) -> None:
        """Reset the blueprint in the MemoryRecording."""
        self.storage.reset_blueprint(add_to_app_default_blueprint)

    def num_msgs(self) -> int:
        """
        The number of pending messages in the MemoryRecording.

        Note: counting the messages will flush the batcher in order to get a deterministic count.
        """
        return self.storage.num_msgs()  # type: ignore[no-any-return]

    def _repr_html_(self) -> Any:
        return self.as_html()
