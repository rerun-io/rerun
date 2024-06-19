"""Helper functions for converting streams to inline html."""

from __future__ import annotations

import logging
from threading import Thread
from time import sleep
from typing import TYPE_CHECKING, Any

from .memory import MemoryRecording, _memory_recording_with_flush_hook

if TYPE_CHECKING:
    from .blueprint import BlueprintLike

from rerun import bindings

from .recording_stream import RecordingStream, get_application_id, get_data_recording

DEFAULT_WIDTH = 950
DEFAULT_HEIGHT = 712


class Viewer:
    def __init__(
        self,
        *,
        width: int = DEFAULT_WIDTH,
        height: int = DEFAULT_HEIGHT,
        recording: RecordingStream | None = None,
        display: bool | None = None,
    ):
        try:
            from rerun_notebook import Viewer as _Viewer
        except ImportError:
            logging.error("Could not import rerun_notebook. Please install `rerun-notebook`.")
            hack: Any = None
            return hack

        self._recording = recording
        self._viewer = _Viewer(
            width=width,
            height=height,
        )

        if display is not None and display:
            from IPython.display import display as do_display

            do_display(self._viewer)

        recording = recording if recording is not None else get_data_recording()
        self._memory_recording = _memory_recording_with_flush_hook(recording, flush_hook=self._flush_hook)

    def _force_flush(self):
        num_msgs = self._memory_recording.num_msgs()
        if num_msgs > 0:
            data = self._memory_recording.drain_as_bytes()
            self._viewer.send_rrd(data)

    def _flush_hook(self, recording: MemoryRecording):
        # we are already in a flush, so don't flush again
        num_msgs = recording._num_msgs_no_flush()
        if num_msgs > 0:
            data = recording._drain_as_bytes_no_flush()
            self._viewer.send_rrd(data)

    def _repr_mimebundle_(self, **kwargs: dict) -> tuple[dict, dict] | None:
        return self._viewer._repr_mimebundle_(**kwargs)

    def _repr_keys(self):
        return self._viewer._repr_keys()


def notebook_show(
    *,
    width: int = DEFAULT_WIDTH,
    height: int = DEFAULT_HEIGHT,
    blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
) -> Viewer:
    """
    Output the Rerun viewer in a notebook using IPython [IPython.core.display.HTML][].

    Parameters
    ----------
    width : int
        The width of the viewer in pixels.
    height : int
        The height of the viewer in pixels.
    blueprint : BlueprintLike
        The blueprint to display in the viewer.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    return Viewer(
        width=width,
        height=height,
        recording=recording,
    )


"""
try:
    from rerun_notebook import Viewer as _Viewer
except ImportError:
    logging.error("Could not import rerun_notebook. Please install `rerun-notebook`.")
    hack: Any = None
    return hack

application_id = get_application_id(recording)
if application_id is None:
    raise ValueError(
        "No application id found. You must call rerun.init before using the notebook APIs, or provide a recording."
    )

# we want the blueprint to come first in the stream,
# so we create a new stream, send a blueprint to it,
# then prepend its output to the existing recording data
output_stream = RecordingStream(
    bindings.new_recording(
        application_id=application_id,
        make_default=False,
        make_thread_default=False,
        default_enabled=True,
    )
)
if blueprint is not None:
    output_stream.send_blueprint(blueprint, make_active=True)  # type: ignore[attr-defined]

data_memory = memory_recording(recording=recording)
output_memory = output_stream.memory_recording()  # type: ignore[attr-defined]

data = output_memory.storage.concat_as_bytes(data_memory.storage)
return Viewer(
    width=width,
    height=height,
    recording=data,
)
"""
