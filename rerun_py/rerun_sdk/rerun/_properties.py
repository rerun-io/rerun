from __future__ import annotations

from typing import TYPE_CHECKING

import rerun_bindings as bindings

from rerun._log import log
from rerun.error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from collections.abc import Iterable

    from rerun._baseclasses import AsComponents, DescribedComponentBatch

    from .recording_stream import RecordingStream


@catch_and_log_exceptions()
def send_property(
    name: str,
    values: AsComponents | Iterable[DescribedComponentBatch],
    recording: RecordingStream | None = None,
) -> None:
    """
    Send a property of the recording.

    Parameters
    ----------
    name:
        Name of the property.

    values:
        Anything that implements the [`rerun.AsComponents`][] interface, usually an archetype,
        or an iterable of (described)component batches.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    entity_path = bindings.new_property_entity_path([name])

    log(entity_path, values, recording=recording, static=True)  # NOLINT


def send_recording_name(name: str, recording: RecordingStream | None = None) -> None:
    """
    Send the name of the recording.

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

    bindings.send_recording_name(name, recording=recording.to_native() if recording is not None else None)


def send_recording_start_time_nanos(nanos: int, recording: RecordingStream | None = None) -> None:
    """
    Send the start time of the recording.

    This timestamp is shown in the Rerun Viewer.

    Parameters
    ----------
    nanos : int
        The start time of the recording in nanoseconds since UNIX epoch.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.send_recording_start_time_nanos(nanos, recording=recording.to_native() if recording is not None else None)
