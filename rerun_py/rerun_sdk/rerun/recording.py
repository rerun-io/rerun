"""Helper functions for displaying Rerun in a Jupyter notebook."""

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
        self, width: int = 950, height: int = 712, app_location: Optional[str] = None, timeout_ms: int = 2000
    ) -> str:
        """
        Show the Rerun viewer in a Jupyter notebook.

        Parameters
        ----------
        width : int
            The width of the viewer in pixels.
        height : int
            The height of the viewer in pixels.
        app_location : str
            The location of the Rerun web viewer.
        timeout_ms : int
            The number of milliseconds to wait for the Rerun web viewer to load.
        """

        if app_location is None:
            app_location = bindings.get_app_url()

        # Use a random presentation ID to avoid collisions when multiple recordings are shown in the same notebook.
        presentation_id = "".join(random.choice(string.ascii_letters) for i in range(6))

        base64_data = base64.b64encode(self.storage.get_rrd_as_bytes()).decode("utf-8")

        html_template = f"""
        <div id="{presentation_id}_rrd" style="display: none;" data-rrd="{base64_data}"></div>
        <div id="{presentation_id}_error" style="display: none;"><p>Timed out waiting for {app_location} to load.</p>
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
            src="{app_location}?url=web_event://&persist=0"
            frameborder="0" style="display: none;" allowfullscreen=""></iframe>
        """

        return html_template

    def show(self, **kwargs: Any) -> Any:
        html = self.as_html(**kwargs)
        try:
            from IPython.core.display import HTML

            return HTML(html)  # type: ignore[no-untyped-call]
        except ImportError:
            logging.warning("Could not import IPython.core.display. Returning raw HTML string instead.")
            return html

    def _repr_html_(self) -> Any:
        return self.as_html()
