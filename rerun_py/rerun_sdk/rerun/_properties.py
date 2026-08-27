from __future__ import annotations

from collections.abc import Iterable
from typing import TYPE_CHECKING

import rerun_bindings as bindings
from rerun._log import log
from rerun.error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    import pyarrow as pa

    from rerun._baseclasses import AsComponents, ComponentDescriptor, DescribedComponentBatch

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
    name:
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
    nanos:
        The start time of the recording in nanoseconds since UNIX epoch.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    bindings.send_recording_start_time_nanos(nanos, recording=recording.to_native() if recording is not None else None)


def build_property_args(
    values: AsComponents | Iterable[DescribedComponentBatch],
) -> dict[ComponentDescriptor, pa.Array]:
    """
    Convert property values into per-component Arrow arrays.

    Accepts either an `AsComponents` object or an iterable of `DescribedComponentBatch`,
    and returns a `{ComponentDescriptor: pyarrow.Array}` mapping ready to be forwarded
    to the native layer.

    Raises
    ------
    TypeError
        If `values` is neither `AsComponents` nor an iterable of `DescribedComponentBatch`.

    """
    batches: Iterable[DescribedComponentBatch]
    if hasattr(values, "as_component_batches"):  # same dispatch as rerun._log.log
        batches = values.as_component_batches()
    elif isinstance(values, Iterable):
        batches = values
    else:
        raise TypeError(f"Expected AsComponents or an iterable of DescribedComponentBatch, got {type(values).__name__}")

    property_args: dict[ComponentDescriptor, pa.Array] = {}
    for b in batches:
        descriptor = b.component_descriptor()
        arrow_array = b.as_arrow_array()
        property_args[descriptor] = arrow_array

    return property_args
