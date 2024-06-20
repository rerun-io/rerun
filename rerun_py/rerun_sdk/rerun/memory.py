"""Helper functions for directly working with recordings."""

from __future__ import annotations

from rerun import bindings

from .recording_stream import RecordingStream


def memory_recording(recording: RecordingStream | None = None) -> MemoryRecording:
    """
    Streams all log-data to a memory buffer.

    This can be used to display the RRD to alternative formats such as html.
    See: [rerun.notebook_show][].

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

    def num_msgs(self) -> int:
        """
        The number of pending messages in the MemoryRecording.

        Note: counting the messages will flush the batcher in order to get a deterministic count.
        """
        return self.storage.num_msgs()  # type: ignore[no-any-return]

    def drain_as_bytes(self) -> bytes:
        """
        Drains the MemoryRecording and returns the data as bytes.

        This will flush the current sink before returning.
        """
        return self.storage.drain_as_bytes()  # type: ignore[no-any-return]
