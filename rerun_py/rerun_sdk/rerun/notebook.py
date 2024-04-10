"""Helper functions for converting streams to inline html."""

from __future__ import annotations

import base64
import logging
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .blueprint import BlueprintLike

from rerun import bindings

from .html_shared import DEFAULT_HEIGHT, DEFAULT_TIMEOUT, DEFAULT_WIDTH, render_html_template
from .memory import memory_recording
from .recording_stream import RecordingStream, get_application_id


def as_html(
    *,
    width: int = DEFAULT_WIDTH,
    height: int = DEFAULT_HEIGHT,
    app_url: str | None = None,
    timeout_ms: int = DEFAULT_TIMEOUT,
    blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
) -> str:
    """
    Generate an HTML snippet that displays the recording in an IFrame.

    For use in contexts such as Jupyter notebooks.

    ⚠️ This will do a blocking flush of the current sink before returning!

    Parameters
    ----------
    width : int
        The width of the viewer in pixels.
    height : int
        The height of the viewer in pixels.
    app_url : str
        Alternative HTTP url to find the Rerun web viewer. This will default to using https://app.rerun.io
        or localhost if [rerun.start_web_viewer_server][] has been called.
    timeout_ms : int
        The number of milliseconds to wait for the Rerun web viewer to load.
    blueprint : BlueprintLike
        The blueprint to display in the viewer.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    application_id = get_application_id(recording)
    if application_id is None:
        raise ValueError(
            "No application id found. You must call rerun.init before using the notebook APIs, or provide a recording."
        )

    if app_url is None:
        app_url = bindings.get_app_url()

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

    base64_data = base64.b64encode(output_memory.storage.concat_as_bytes(data_memory.storage)).decode("utf-8")

    return render_html_template(
        base64_data=base64_data,
        app_url=app_url,
        timeout_ms=timeout_ms,
        width=width,
        height=height,
    )


def notebook_show(
    *,
    width: int = DEFAULT_WIDTH,
    height: int = DEFAULT_HEIGHT,
    app_url: str | None = None,
    timeout_ms: int = DEFAULT_TIMEOUT,
    blueprint: BlueprintLike | None = None,
    recording: RecordingStream | None = None,
) -> Any:
    """
    Output the Rerun viewer in a notebook using IPython [IPython.core.display.HTML][].

    Parameters
    ----------
    width : int
        The width of the viewer in pixels.
    height : int
        The height of the viewer in pixels.
    app_url : str
        Alternative HTTP url to find the Rerun web viewer. This will default to using https://app.rerun.io
        or localhost if [rerun.start_web_viewer_server][] has been called.
    timeout_ms : int
        The number of milliseconds to wait for the Rerun web viewer to load.
    blueprint : BlueprintLike
        The blueprint to display in the viewer.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    html = as_html(
        width=width, height=height, app_url=app_url, timeout_ms=timeout_ms, blueprint=blueprint, recording=recording
    )
    try:
        from IPython.core.display import HTML

        return HTML(html)  # type: ignore[no-untyped-call]
    except ImportError:
        logging.warning("Could not import IPython.core.display. Returning raw HTML string instead.")
        return html
