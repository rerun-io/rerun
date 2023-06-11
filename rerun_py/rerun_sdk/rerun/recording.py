"""Helper functions for directly working with recordings."""
from __future__ import annotations

import base64
import logging
import random
import string
from typing import Any

from rerun import bindings

DEFAULT_WIDTH = 950
DEFAULT_HEIGHT = 712
DEFAULT_TIMEOUT = 2000


class MemoryRecording:
    def __init__(self, storage: bindings.PyMemorySinkStorage) -> None:
        self.storage = storage

    def reset_data(self) -> None:
        """Reset the data in the MemoryRecording."""
        self.storage.reset_data()

    def reset_blueprint(self, add_to_app_default_blueprint: bool = False) -> None:
        """Reset the blueprint in the MemoryRecording."""
        self.storage.reset_blueprint(add_to_app_default_blueprint)  # type: ignore[no-untyped-def]

    def as_html(
        self,
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

        # Use a random presentation ID to avoid collisions when multiple recordings are shown in the same notebook.
        presentation_id = "".join(random.choice(string.ascii_letters) for i in range(6))

        if other:
            other = other.storage
        base64_data = base64.b64encode(self.storage.concat_as_bytes(other)).decode("utf-8")

        html_template = f"""
        <div id="{presentation_id}_rrd" style="display: none;" data-rrd="{base64_data}"></div>
        <div id="{presentation_id}_error" style="display: none;"><p>Timed out waiting for {app_url} to load.</p>
        <p>Consider using <code>rr.start_web_viewer_server()</code></p></div>
        <script>
            {presentation_id}_timeout = setTimeout(() => {{
                document.getElementById("{presentation_id}_error").style.display = 'block';
            }}, {timeout_ms});

            window.addEventListener("message", function(rrd) {{
                return async function {presentation_id}_onIframeReady(event) {{
                    var iframe = document.getElementById("{presentation_id}_iframe");
                    if (event.source === iframe.contentWindow) {{
                        clearTimeout({presentation_id}_timeout);
                        document.getElementById("{presentation_id}_error").style.display = 'none';
                        iframe.style.display = 'inline';
                        window.removeEventListener("message", {presentation_id}_onIframeReady);
                        iframe.contentWindow.postMessage((await rrd), "*");
                    }}
                }}
            }}(async function() {{
                await new Promise(r => setTimeout(r, 0));
                var div = document.getElementById("{presentation_id}_rrd");
                var base64Data = div.dataset.rrd;
                var intermediate = atob(base64Data);
                var buff = new Uint8Array(intermediate.length);
                for (var i = 0; i < intermediate.length; i++) {{
                    buff[i] = intermediate.charCodeAt(i);
                }}
                return buff;
            }}()));
        </script>
        <iframe id="{presentation_id}_iframe" width="{width}" height="{height}"
            src="{app_url}?url=web_event://&persist=0"
            frameborder="0" style="display: none;" allowfullscreen=""></iframe>
        """

        return html_template

    def show(
        self,
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
