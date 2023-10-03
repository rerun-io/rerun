from __future__ import annotations

from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.archetypes import Clear
from rerun.recording_stream import RecordingStream


@deprecated(
    """Please migrate to `rr.log(…, rr.Clear(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
def log_cleared(
    entity_path: str,
    *,
    recursive: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Indicate that an entity at a given path should no longer be displayed.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Clear][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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
    return log(entity_path, Clear(recursive=recursive), recording=recording)
