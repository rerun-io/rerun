from __future__ import annotations

import importlib.metadata
import logging
import pathlib
import sys
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


class Viewer(anywidget.AnyWidget):
    _esm = pathlib.Path(__file__).parent / "static" / "widget.js"
    _css = pathlib.Path(__file__).parent / "static" / "widget.css"

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
                    logging.warning("Timed out waiting for viewer to become ready.")
                    return
                poll(1)
                time.sleep(0.1)
