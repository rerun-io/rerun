from __future__ import annotations

from typing import TYPE_CHECKING

from typing_extensions import deprecated

from rerun.experimental import Chunk
from rerun_bindings import recording_from_chunks

if TYPE_CHECKING:
    from collections.abc import Generator, Iterable
    from pathlib import Path

    from rerun.catalog import Schema
    from rerun_bindings import RecordingInternal, RRDArchiveInternal


@deprecated(
    "Recording is deprecated since 0.32. Use rerun.experimental.RrdReader instead.",
)
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

    def chunks(self) -> Generator[Chunk, None, None]:
        """Iterate over all physical chunks in this recording."""

        for chunk_internal in self._internal.chunks():
            yield Chunk(chunk_internal)

    @staticmethod
    @deprecated(
        "Recording.from_chunks is deprecated since 0.32. Use "
        "LazyChunkStream.from_iter(chunks) and pass application_id and recording_id "
        "to the destination (e.g. .write_rrd(path, application_id=…, recording_id=…)).",
    )
    def from_chunks(chunks: Iterable[Chunk], application_id: str, recording_id: str) -> Recording:  # ty:ignore[deprecated]
        """
        Create a new recording from an iterable of chunks.

        Parameters
        ----------
        chunks:
            An iterable of chunks to include in the recording.
        application_id:
            The application ID for the new recording.
        recording_id:
            The recording ID for the new recording.

        Returns
        -------
        Recording
            The newly created recording.

        """

        return Recording(recording_from_chunks((c._internal for c in chunks), application_id, recording_id))  # ty:ignore[deprecated]

    def save(self, path: str | Path) -> None:
        """Save this recording to an RRD file."""
        self._internal.save(str(path))


@deprecated(
    "RRDArchive is deprecated since 0.32. Use rerun.experimental.RrdReader instead.",
)
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

    def all_recordings(self) -> list[Recording]:  # ty:ignore[deprecated]
        """All the recordings in the archive."""
        return [Recording(r) for r in self._internal.all_recordings()]  # ty:ignore[deprecated]
