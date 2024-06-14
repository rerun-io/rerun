"""Helper functions for converting streams to inline html."""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Any

from .memory import memory_recording

if TYPE_CHECKING:
    from .blueprint import BlueprintLike

from rerun import bindings

from .recording_stream import RecordingStream, get_application_id

DEFAULT_WIDTH = 950
DEFAULT_HEIGHT = 712

if TYPE_CHECKING:
    try:
        from rerun_notebook import Viewer
    except ImportError:
        pass


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

    try:
        from rerun_notebook import Viewer

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
        viewer = Viewer()
        viewer.send_rrd(data)
        return viewer

    except ImportError:
        logging.error("Could not import rerun_notebook. Please install `rerun-notebook`.")

        hack: Any = None
        return hack
