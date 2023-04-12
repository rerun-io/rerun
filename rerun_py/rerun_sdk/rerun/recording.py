"""Helper functions for displaying Rerun in a Jupyter notebook."""

import base64
import logging
import random
import string
from typing import Any, Optional

from rerun import bindings


class MemoryRecording:
    def __init__(self, storage: bindings.PyMemoryRecording) -> None:
        self.storage = storage

    def inline_show(
        self, width: int = 950, height: int = 712, app_location: Optional[str] = None, timeout_ms: int = 2000
    ) -> Any:
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

        random_string = "".join(random.choice(string.ascii_letters) for i in range(6))

        base64_data = base64.b64encode(self.storage.get_rrd_as_bytes()).decode("utf-8")

        html_template = f"""
        <div id="{random_string}_rrd" style="display: none;">{base64_data}</div>
        <div id="{random_string}_error" style="display: none;"><p>Timed out waiting for {app_location} to load.</p>
        <p>Consider using <code>rr.self_host_assets()</code></p></div>
        <script>
            timeout_{random_string} = setTimeout(() => {{
                document.getElementById("{random_string}_error").style.display = 'block';
            }}, {timeout_ms});

            window.addEventListener("message", function(rrd) {{
                return async function onIframeReady_{random_string}(event) {{
                    var iframe = document.getElementById("{random_string}");
                    if (event.source === iframe.contentWindow) {{
                        clearTimeout(timeout_{random_string});
                        document.getElementById("{random_string}_error").style.display = 'none';
                        iframe.style.display = 'inline';
                        window.removeEventListener("message", onIframeReady_{random_string});
                        iframe.contentWindow.postMessage((await rrd), "*");
                    }}
                }}
            }}(async function() {{
                await new Promise(r => setTimeout(r, 0));
                var div = document.getElementById("{random_string}_rrd");
                var base64Data = div.textContent;
                var intermediate = atob(base64Data);
                var buff = new Uint8Array(intermediate.length);
                for (var i = 0; i < intermediate.length; i++) {{
                    buff[i] = intermediate.charCodeAt(i);
                }}
                return buff;
            }}()));
        </script>
        <iframe id="{random_string}" width="{width}" height="{height}" src="{app_location}?url=web_event://&persist=0"
            frameborder="0" style="display: none;" allowfullscreen=""></iframe>
        """
        try:
            from IPython.core.display import HTML

            return HTML(html_template)  # type: ignore[no-untyped-call]
        except ImportError:
            logging.warning("Could not import IPython.core.display. Returning HTML string instead.")

        return html_template
