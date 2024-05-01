"""Helper functions for directly working with recordings."""

from __future__ import annotations

import base64
import logging
from typing import Any

from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun import bindings

from .html_shared import DEFAULT_HEIGHT, DEFAULT_TIMEOUT, DEFAULT_WIDTH, render_html_template
from .recording_stream import RecordingStream


def memory_recording(recording: RecordingStream | None = None) -> MemoryRecording:
    """
    Streams all log-data to a memory buffer.

    This can be used to display the RRD to alternative formats such as html.
    See: [rerun.MemoryRecording.as_html][].

    Parameters
    ----------
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    Returns
    -------
    MemoryRecording
        A memory recording object that can be used to read the data.

    """

    recording = RecordingStream.to_native(recording)
    return MemoryRecording(bindings.memory_recording(recording=recording))


class MemoryRecording:
    """A recording that stores data in memory."""

    def __init__(self, storage: bindings.PyMemorySinkStorage) -> None:
        self.storage = storage

    def num_msgs(self) -> int:
        """
        The number of pending messages in the MemoryRecording.

        Note: counting the messages will flush the batcher in order to get a deterministic count.
        """
        return self.storage.num_msgs()  # type: ignore[no-any-return]

    def drain_as_bytes(self) -> bytes:
        """
        Drains the MemoryRecording and returns the data as bytes.

        This will flush the current sink before returning.
        """
        return self.storage.drain_as_bytes()  # type: ignore[no-any-return]

    @deprecated("Please use rerun.notebook_show() instead.")
    def as_html(
        self,
        *,
        width: int = DEFAULT_WIDTH,
        height: int = DEFAULT_HEIGHT,
        app_url: str | None = None,
        timeout_ms: int = DEFAULT_TIMEOUT,
        other: MemoryRecording | None = None,
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
        other: MemoryRecording
            An optional MemoryRecording to merge with this one.

        """

        if app_url is None:
            app_url = bindings.get_app_url()

        if other:
            other = other.storage
        base64_data = base64.b64encode(self.storage.concat_as_bytes(other)).decode("utf-8")

        return """
<div style="background-color: #ffcccb; color: #8b0000; padding: 10px; border: 1px solid #8b0000; border-radius: 5px; margin: 20px;">
  Direct rendering of MemoryRecording has been deprecated. Please prefer rerun.notebook_show().
</div>
""" + render_html_template(
            base64_data=base64_data,
            app_url=app_url,
            timeout_ms=timeout_ms,
            width=width,
            height=height,
        )

    @deprecated("Please use rerun.notebook_show() instead.")
    def show(
        self,
        *,
        other: MemoryRecording | None = None,
        width: int = DEFAULT_WIDTH,
        height: int = DEFAULT_HEIGHT,
        app_url: str | None = None,
        timeout_ms: int = DEFAULT_TIMEOUT,
    ) -> Any:
        """
        Output the Rerun viewer using IPython [IPython.core.display.HTML][].

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
        other: MemoryRecording
            An optional MemoryRecording to merge with this one.

        """
        html = self.as_html(width=width, height=height, app_url=app_url, timeout_ms=timeout_ms, other=other)
        try:
            from IPython.core.display import HTML

            return HTML(html)  # type: ignore[no-untyped-call]
        except ImportError:
            logging.warning("Could not import IPython.core.display. Returning raw HTML string instead.")
            return html

    def _repr_html_(self) -> Any:
        return self.as_html()
