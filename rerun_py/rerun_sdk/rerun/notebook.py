"""Helper functions for converting streams to inline html."""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .blueprint import BlueprintLike

from rerun import bindings

from .recording_stream import RecordingStream, get_data_recording

DEFAULT_WIDTH = 950
DEFAULT_HEIGHT = 712


class Viewer:
    """
    A viewer embeddable in a notebook.

    This viewer is a wrapper around the [`rerun_notebook.Viewer`][] widget.
    """

    def __init__(
        self,
        *,
        width: int = DEFAULT_WIDTH,
        height: int = DEFAULT_HEIGHT,
        blueprint: BlueprintLike | None = None,
        recording: RecordingStream | None = None,
        display: bool = False,
    ):
        """
        Create a new Rerun viewer for use in a notebook.

        Any data logged to the recording after initialization will be sent directly to the viewer.

        Parameters
        ----------
        width : int
            The width of the viewer in pixels.
        height : int
            The height of the viewer in pixels.
        recording:
            Specifies the [`rerun.RecordingStream`][] to use.
            If left unspecified, defaults to the current active data recording, if there is one.
            See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
        blueprint:
            A blueprint object to send to the viewer.
            It will be made active and set as the default blueprint in the recording.

            Setting this is equivalent to calling [`rerun.send_blueprint`][] before initializing the viewer.
        display : bool
            Whether to display the viewer in the current notebook cell
            immediately after initialization.
            Defaults to `False`.

        """

        try:
            from rerun_notebook import Viewer as _Viewer  # type: ignore[attr-defined]
        except ImportError:
            logging.error("Could not import rerun_notebook. Please install `rerun-notebook`.")
            hack: Any = None
            return hack  # type: ignore[no-any-return]

        recording = get_data_recording(recording)
        if recording is None:
            raise ValueError("No recording specified and no active recording found")

        self._recording = recording

        if blueprint is not None:
            self._recording.send_blueprint(blueprint)  # type: ignore[attr-defined]

        self._viewer = _Viewer(
            width=width,
            height=height,
        )

        if display:
            self.display()

        bindings.set_callback_sink(
            recording=RecordingStream.to_native(self._recording),
            callback=self._flush_hook,
        )

    def display(self) -> None:
        """Display the viewer in a notebook cell."""

        from IPython.display import display

        display(self._viewer)

    def _flush_hook(self, data: bytes) -> None:
        self._viewer.send_rrd(data)

    def _repr_mimebundle_(self, **kwargs: dict) -> tuple[dict, dict] | None:  # type: ignore[type-arg]
        return self._viewer._repr_mimebundle_(**kwargs)  # type: ignore[no-any-return]

    def _repr_keys(self):  # type: ignore[no-untyped-def]
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

    Any data logged to the recording after initialization will be sent directly to the viewer.

    Parameters
    ----------
    width : int
        The width of the viewer in pixels.
    height : int
        The height of the viewer in pixels.
    blueprint : BlueprintLike
        A blueprint object to send to the viewer.
        It will be made active and set as the default blueprint in the recording.

        Setting this is equivalent to calling [`rerun.send_blueprint`][] before initializing the viewer.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    return Viewer(
        width=width,
        height=height,
        blueprint=blueprint,
        recording=recording,
    )
