"""Helper functions for directly working with recordings."""

from __future__ import annotations

from typing import Callable

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

    return _memory_recording_with_flush_hook(recording=recording)


def _memory_recording_with_flush_hook(
    recording: RecordingStream | None = None,
    flush_hook: Callable[[MemoryRecording], None] | None = None,
) -> MemoryRecording:
    recording = RecordingStream.to_native(recording)

    if flush_hook is not None:
        hook = lambda storage: flush_hook(MemoryRecording(storage))  # noqa: E731
    else:
        hook = None

    return MemoryRecording(
        bindings.memory_recording(
            recording=recording,
            flush_hook=hook,
        )
    )


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

    def _num_msgs_no_flush(self) -> int:
        return self.storage.num_msgs_no_flush()

    def _drain_as_bytes_no_flush(self) -> bytes:
        return self.storage.drain_as_bytes_no_flush()
