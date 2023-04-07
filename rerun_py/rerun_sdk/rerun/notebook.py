"""Helper functions for displaying Rerun in a Jupyter notebook."""

import base64
import logging
import random
import string
from typing import Any, Optional

from rerun import bindings


def inline_show(width: int = 950, height: int = 712, app_location: Optional[str] = None) -> Any:
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
    """

    # TODO(jleibs): Resolve this URL dynamically.
    if app_location is None:
        app_location = "https://app.rerun.io/commit/4797e6a/index.html"

    random_string = "".join(random.choice(string.ascii_letters) for i in range(6))

    base64_data = base64.b64encode(bindings.dump_rrd_as_bytes()).decode("utf-8")

    html_template = f"""
    <div id="{random_string}_rrd" style="display: none;">{base64_data}</div>
    <script>
        window.addEventListener("message", function(rrd) {{
            return async function onIframeReady_{random_string}(event) {{
                var iframe = document.getElementById("{random_string}");
                if (event.source === iframe.contentWindow) {{
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
    <iframe id="{random_string}" width="{width}" height="{height}" src="{app_location}?url=web_event://"
        frameborder="0" allowfullscreen=""></iframe>
    """
    try:
        from IPython.core.display import HTML

        return HTML(html_template)  # type: ignore[no-untyped-call]
    except ImportError:
        logging.warning("Could not import IPython.core.display. Returning HTML string instead.")

    return html_template
