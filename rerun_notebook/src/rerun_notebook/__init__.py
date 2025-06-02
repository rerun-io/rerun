from __future__ import annotations

import http.client
import importlib.metadata
import logging
import os
import pathlib
import time
import urllib.parse
from collections.abc import Mapping
from concurrent.futures import ThreadPoolExecutor
from typing import Any, Callable, Literal

import anywidget
import jupyter_ui_poll
import traitlets

try:
    __version__ = importlib.metadata.version("rerun_notebook")
except importlib.metadata.PackageNotFoundError:
    __version__ = "unknown"


Panel = Literal["top", "blueprint", "selection", "time"]
PanelState = Literal["expanded", "collapsed", "hidden"]

STATIC_DIR = pathlib.Path(__file__).parent / "static"
WIDGET_PATH = STATIC_DIR / "widget.js"
WASM_PATH = STATIC_DIR / "re_viewer_bg.wasm"
CSS_PATH = STATIC_DIR / "widget.css"

# We need to bootstrap the value of ESM_MOD before the Viewer class is instantiated.
# This is because AnyWidget will process the resource files during `__init_subclass__`

# We allow customization through the `RERUN_NOTEBOOK_ASSET` environment variable.
# The default value is hosted at `app.rerun.io`.
# The value can be set to `serve-local` to use the hosted asset server.
# The value can be set to `inline` to use the local widget.js file.
# The value can be set to a URL to use a custom asset server.
#  One way to run a custom asset server is to run `python -m rerun_notebook serve`.
ASSET_MAGIC_SERVE = "serve-local"
ASSET_MAGIC_INLINE = "inline"


def _buffer_to_data_url(binary: bytes) -> str:
    import base64
    import gzip

    gz = gzip.compress(binary)
    b64 = base64.b64encode(gz).decode()

    return f"data:application/octet-stream;gzip;base64,{b64}"


def _inline_widget():
    """
    For `RERUN_NOTEBOOK_ASSET=inline`, we need a single file to pass to `anywidget`.

    This function loads both the JS and Wasm files, then inlines the Wasm into the JS.

    The Wasm is stored in the JS file as a (gzipped) base64 string, from which we can
    create a fake Response for the JS to load as a Wasm module.
    """
    print("Loading inline widget due to RERUN_NOTEBOOK_ASSET=inline")

    wasm = WASM_PATH.read_bytes()
    data_url = _buffer_to_data_url(wasm)

    js = WIDGET_PATH.read_text()
    fetch_viewer_wasm = f"""
    async function compressed_data_url_to_buffer(dataUrl) {{
        const response = await fetch(dataUrl);
        const blob = await response.blob();

        let ds = new DecompressionStream("gzip");
        let decompressedStream = blob.stream().pipeThrough(ds);

        return new Response(decompressedStream).arrayBuffer();
    }}
    const dataUrl = "{data_url}";
    const buffer = await compressed_data_url_to_buffer(dataUrl);
    return new Response(buffer, {{ "headers": {{ "Content-Type": "application/wasm" }} }});
    """

    inline_marker = "//!<INLINE-MARKER>"
    inline_start = js.find(inline_marker) + len(inline_marker)
    inline_end = js.find(inline_marker, inline_start)
    js = js[:inline_start] + fetch_viewer_wasm + js[inline_end:]

    return js


def _is_url_accessible(url: urllib.parse.ParseResult) -> tuple[urllib.parse.ParseResult, bool]:
    conn = http.client.HTTPSConnection(url.netloc) if url.scheme == "https" else http.client.HTTPConnection(url.netloc)
    try:
        conn.request("HEAD", url.path or "/")
        res = conn.getresponse()
        return url, res.status == 200
    except Exception:
        return url, False
    finally:
        conn.close()


def _set_parsed_url_filename(url: urllib.parse.ParseResult, filename: str) -> urllib.parse.ParseResult:
    path_parts = url.path.split("/")
    path_parts[-1] = filename
    new_path = "/".join(path_parts)
    return url._replace(path=new_path)


def _check_if_assets_accessible(widget_url: str):
    parsed_url = urllib.parse.urlparse(widget_url)

    assert parsed_url.path.endswith("widget.js")
    urls_to_check = [
        parsed_url,
        _set_parsed_url_filename(parsed_url, "re_viewer_bg.wasm"),
    ]

    with ThreadPoolExecutor(max_workers=2) as e:
        results = list(e.map(_is_url_accessible, urls_to_check))

    success = True
    for url, exists in results:
        if not exists:
            success = False
            print(f'"{url.geturl()}" is not accessible')
    if not success:
        raise ValueError(
            f"One or more asset URLs are inaccessible. Consult https://pypi.org/project/rerun-notebook/{__version__}/ for details."
        )


ASSET_ENV = os.environ.get("RERUN_NOTEBOOK_ASSET", None)
if ASSET_ENV is None:
    # if we're in the `rerun` repository, default to `serve-local`
    # as we don't upload widgets for `+dev` versions anywhere
    if "RERUN_DEV_ENVIRONMENT" in os.environ:
        ASSET_ENV = "serve-local"
    else:
        ASSET_ENV = f"https://app.rerun.io/version/{__version__}/widget.js"

if ASSET_ENV == ASSET_MAGIC_SERVE:  # localhost widget
    from .asset_server import serve_assets

    bound_addr = serve_assets(background=True)
    ESM_MOD: str | pathlib.Path = f"http://localhost:{bound_addr[1]}/widget.js"
elif ASSET_ENV == ASSET_MAGIC_INLINE:  # inline widget
    # in this case, `ESM_MOD` is the contents of a file, not its path.
    ESM_MOD = _inline_widget()
else:  # remote widget
    ESM_MOD = ASSET_ENV
    # note that the JS expects the Wasm binary to exist at the same path as itself
    if not (ASSET_ENV.startswith(("http://", "https://"))):
        raise ValueError(f"RERUN_NOTEBOOK_ASSET should be a URL starting with http or https. Found: {ASSET_ENV}")
    if not (ASSET_ENV.endswith("widget.js")):
        raise ValueError(f"RERUN_NOTEBOOK_ASSET should be a URL pointing to a `widget.js` file. Found: {ASSET_ENV}")

    _check_if_assets_accessible(ASSET_ENV)


class Viewer(anywidget.AnyWidget):  # type: ignore[misc]
    _esm = ESM_MOD
    _css = CSS_PATH

    _width = traitlets.Int(allow_none=True).tag(sync=True)
    _height = traitlets.Int(allow_none=True).tag(sync=True)

    _url = traitlets.Unicode(allow_none=True).tag(sync=True)

    _panel_states = traitlets.Dict(
        key_trait=traitlets.Unicode(),
        value_trait=traitlets.Unicode(),
        allow_none=True,
    ).tag(sync=True)

    _ready = False
    _data_queue: list[bytes]
    _table_queue: list[bytes]

    _time_ctrl = traitlets.Tuple(
        traitlets.Unicode(allow_none=True),
        traitlets.Int(allow_none=True),
        traitlets.Bool(),
        allow_none=True,
    ).tag(sync=True)
    _recording_id = traitlets.Unicode(allow_none=True).tag(sync=True)

    _fallback_token = traitlets.Unicode(allow_none=True).tag(sync=True)

    _raw_event_callbacks: list[Callable[[str], None]] = []

    def __init__(
        self,
        *,
        width: int | None = None,
        height: int | None = None,
        url: str | None = None,
        panel_states: Mapping[Panel, PanelState] | None = None,
        fallback_token: str | None = None,
        **kwargs: Any,
    ) -> None:
        super().__init__(**kwargs)

        self._width = width
        self._height = height
        self._url = url
        self._data_queue = []
        self._table_queue = []

        if panel_states:
            self.update_panel_states(panel_states)

        if fallback_token:
            self._fallback_token = fallback_token

        def handle_msg(widget: Any, content: Any, buffers: list[bytes]) -> None:
            if isinstance(content, str):
                if content == "ready":
                    self._on_ready()
                else:
                    for callback in self._raw_event_callbacks:
                        callback(content)

        self.on_msg(handle_msg)

    def _on_ready(self) -> None:
        self._ready = True

        for data in self._data_queue:
            self.send_rrd(data)
        self._data_queue.clear()

        for data in self._table_queue:
            self.send_table(data)
        self._table_queue.clear()

    def send_rrd(self, data: bytes) -> None:
        """Send a recording to the viewer."""

        if not self._ready:
            self._data_queue.append(data)
            return

        self.send({"type": "rrd"}, buffers=[data])

    def send_table(self, data: bytes) -> None:
        if not self._ready:
            self._table_queue.append(data)
            return

        self.send({"type": "table"}, buffers=[data])

    def block_until_ready(self, timeout: float = 10.0) -> None:
        """Block until the viewer is ready."""

        start = time.time()

        with jupyter_ui_poll.ui_events() as poll:
            while self._ready is False:
                if time.time() - start > timeout:
                    logging.warning(
                        f"Timed out waiting for viewer to load. Consult https://pypi.org/project/rerun-notebook/{__version__}/ for details."
                    )
                    return
                poll(1)
                time.sleep(0.1)

    def update_panel_states(self, panel_states: Mapping[Panel, PanelState | Literal["default"]]) -> None:
        new_panel_states = dict(self._panel_states.items()) if self._panel_states else {}
        for panel, state in panel_states.items():
            if state == "default":
                new_panel_states.pop(panel, None)
            else:
                new_panel_states[panel] = state
        self._panel_states = new_panel_states

    def set_time_ctrl(self, timeline: str | None, time: int | None, play: bool) -> None:
        self._time_ctrl = (timeline, time, play)

    def set_active_recording(self, recording_id: str) -> None:
        self._recording_id = recording_id

    def _on_raw_event(self, callback: Callable[[str], None]) -> None:
        """Register a set of callbacks with this instance of the Viewer."""
        # TODO(jan): maybe allow unregister by making this a map instead
        self._raw_event_callbacks.append(callback)
