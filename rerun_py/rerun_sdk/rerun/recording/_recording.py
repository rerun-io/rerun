from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun.catalog import Schema
    from rerun_bindings import RecordingInternal, RRDArchiveInternal


class Recording:
    """
    A single Rerun recording.

    This can be loaded from an RRD file using [`load_recording()`][rerun.recording.load_recording].

    A recording is a collection of data that was logged to Rerun. This data is organized
    as a column for each index (timeline) and each entity/component pair that was logged.

    You can examine the [`.schema()`][rerun.recording.Recording.schema] of the recording to see
    what data is available.
    """

    _internal: RecordingInternal

    def __init__(self, inner: RecordingInternal) -> None:
        self._internal = inner

    def schema(self) -> Schema:
        """The schema describing all the columns available in the recording."""
        from rerun.catalog import Schema

        return Schema(self._internal.schema())

    def recording_id(self) -> str:
        """The recording ID of the recording."""
        return self._internal.recording_id()

    def application_id(self) -> str:
        """The application ID of the recording."""
        return self._internal.application_id()


class RRDArchive:
    """
    An archive loaded from an RRD.

    RRD archives may include 1 or more recordings or blueprints.
    """

    _internal: RRDArchiveInternal

    def __init__(self, inner: RRDArchiveInternal) -> None:
        self._internal = inner

    def num_recordings(self) -> int:
        """The number of recordings in the archive."""
        return self._internal.num_recordings()

    def all_recordings(self) -> list[Recording]:
        """All the recordings in the archive."""
        return [Recording(r) for r in self._internal.all_recordings()]
