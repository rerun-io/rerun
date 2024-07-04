from __future__ import annotations

import importlib.metadata
import logging
import os
import pathlib
import time
from typing import Any, Literal

import anywidget
import jupyter_ui_poll
import traitlets

try:
    __version__ = importlib.metadata.version("rerun_notebook")
except importlib.metadata.PackageNotFoundError:
    __version__ = "unknown"


Panel = Literal["top", "blueprint", "selection", "time"]
PanelState = Literal["expanded", "collapsed", "hidden"]

WIDGET_PATH = pathlib.Path(__file__).parent / "static" / "widget.js"
CSS_PATH = pathlib.Path(__file__).parent / "static" / "widget.css"

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

ASSET_ENV = os.environ.get("RERUN_NOTEBOOK_ASSET", f"https://app.rerun.io/version/{__version__}/widget.js")

if ASSET_ENV == ASSET_MAGIC_SERVE:
    from .asset_server import serve_assets

    bound_addr = serve_assets(background=True)
    ESM_MOD = f"http://localhost:{bound_addr[1]}/widget.js"
elif ASSET_ENV == ASSET_MAGIC_INLINE:
    ESM_MOD = WIDGET_PATH
else:
    ESM_MOD = ASSET_ENV
    if not (ASSET_ENV.startswith("http://") or ASSET_ENV.startswith("https://")):
        raise ValueError(f"RERUN_NOTEBOOK_ASSET should be a URL starting with http or https. Found: {ASSET_ENV}")


class Viewer(anywidget.AnyWidget):
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

    def __init__(
        self,
        *,
        width: int | None = None,
        height: int | None = None,
        url: str | None = None,
        panel_states: dict[Panel, PanelState] | None = None,
        **kwargs,
    ):
        super().__init__(**kwargs)

        self._width = width
        self._height = height
        self._url = url
        self._panel_states = panel_states
        self._data_queue = []

        def handle_msg(widget: Any, content: Any, buffers: list[bytes]) -> None:
            if isinstance(content, str) and content == "ready":
                self._on_ready()

        self.on_msg(handle_msg)

    def _on_ready(self):
        self._ready = True
        for data in self._data_queue:
            self.send_rrd(data)
        self._data_queue.clear()

    def send_rrd(self, data: bytes) -> None:
        """Send a recording to the viewer."""

        if not self._ready:
            self._data_queue.append(data)
            return

        self.send({"type": "rrd"}, buffers=[data])

    def block_until_ready(self, timeout=5.0) -> None:
        """Block until the viewer is ready."""

        start = time.time()

        with jupyter_ui_poll.ui_events() as poll:
            while self._ready is False:
                if time.time() - start > timeout:
                    logging.warning(
                        f"""Timed out waiting for viewer to become ready. Make sure: {ESM_MOD} is accessible.
If not, consider setting `RERUN_NOTEBOOK_ASSET`. Consult https://pypi.org/project/rerun-notebook/{__version__}/ for details.
"""
                    )
                    return
                poll(1)
                time.sleep(0.1)
