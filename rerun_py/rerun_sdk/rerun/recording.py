"""Helper functions for directly working with recordings."""

import base64
import logging
import random
import string
from typing import Any, Optional

from rerun import bindings


class MemoryRecording:
    def __init__(self, storage: bindings.PyMemorySinkStorage) -> None:
        self.storage = storage

    def as_html(
        self, width: int = 950, height: int = 712, app_url: Optional[str] = None, timeout_ms: int = 2000
    ) -> str:
        """
        Generate an HTML snippet that displays the recording in an IFrame.

        For use in contexts such as Jupyter notebooks.

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
        """

        if app_url is None:
            app_url = bindings.get_app_url()

        # Use a random presentation ID to avoid collisions when multiple recordings are shown in the same notebook.
        presentation_id = "".join(random.choice(string.ascii_letters) for i in range(6))

        base64_data = base64.b64encode(self.storage.get_rrd_as_bytes()).decode("utf-8")

        html_template = f"""
        <div id="{presentation_id}_rrd" style="display: none;" data-rrd="{base64_data}"></div>
        <div id="{presentation_id}_error" style="display: none;"><p>Timed out waiting for {app_url} to load.</p>
        <p>Consider using <code>rr.self_host_assets()</code></p></div>
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

    def show(self, **kwargs: Any) -> Any:
        """Output the Rerun viewer using IPython [IPython.core.display.HTML][]."""
        html = self.as_html(**kwargs)
        try:
            from IPython.core.display import HTML

            return HTML(html)  # type: ignore[no-untyped-call]
        except ImportError:
            logging.warning("Could not import IPython.core.display. Returning raw HTML string instead.")
            return html

    def _repr_html_(self) -> Any:
        return self.as_html()
