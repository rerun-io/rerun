from __future__ import annotations

from collections.abc import Iterable
from typing import Optional

import rerun_bindings as bindings

from rerun._baseclasses import AsComponents, DescribedComponentBatch
from rerun._log import log
from rerun.error_utils import catch_and_log_exceptions

from .recording_stream import RecordingStream


@catch_and_log_exceptions()
def set_properties(
    properties: AsComponents | Iterable[DescribedComponentBatch],
    entity_path: Optional[str | list[object]] = None,
    recording: RecordingStream | None = None,
) -> None:
    """
    Set the properties of the recording.

    These are builtin recording properties known to the Rerun viewer.

    Parameters
    ----------
    entity_path:
        Path to the entity in the recording properties.

    properties :
        Anything that implements the [`rerun.AsComponents`][] interface, usually an archetype,
        or an iterable of (described)component batches.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    if entity_path is None:
        entity_path = []

    if isinstance(entity_path, list):
        entity_path = bindings.new_property_entity_path([str(part) for part in entity_path])

    log(entity_path, properties, recording=recording, static=True)


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


def set_start_time_nanos(nanos: int, recording: RecordingStream | None = None) -> None:
    """
    Set the start time of the recording.

    This timestamp is shown in the Rerun Viewer.

    Parameters
    ----------
    nanos : int
        The start time of the recording.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.set_start_time_nanos(nanos, recording=recording.to_native() if recording is not None else None)
