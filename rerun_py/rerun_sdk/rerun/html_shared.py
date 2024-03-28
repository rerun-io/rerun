from __future__ import annotations

import uuid

DEFAULT_WIDTH = 950
DEFAULT_HEIGHT = 712
DEFAULT_TIMEOUT = 2000


def render_html_template(base64_data: str, app_url: str, timeout_ms: int, width: int, height: int) -> str:
    # Use a random presentation ID to avoid collisions when multiple recordings are shown in the same notebook.
    presentation_id = "_" + uuid.uuid4().hex

    return f"""<div id="{presentation_id}_rrd" style="display: none;" data-rrd="{base64_data}"></div>
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
    src="{app_url}?url=web_event://&persist=0&notebook=1"
    frameborder="0" style="display: none;" allowfullscreen=""></iframe>
"""
