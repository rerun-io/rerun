from __future__ import annotations

import importlib.metadata
import pathlib
from typing import Any, Literal

import anywidget
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

    _url = traitlets.Unicode(allow_none=True).tag(sync=True)
    _panel_states = traitlets.Dict(
        key_trait=traitlets.Unicode(),
        value_trait=traitlets.Unicode(),
        allow_none=True,
    ).tag(sync=True)

    _ready = False
    _rrd_buffer: list[bytes] = []

    def __init__(
        self,
        *,
        url: str | None = None,
        panel_states: dict[Panel, PanelState] | None = None,
        **kwargs,
    ):
        super().__init__(**kwargs)

        if url is not None:
            self._url = url

        if panel_states is not None:
            self._panel_states = panel_states

        def handle_msg(widget: Viewer, content: Any, buffers: list[bytes]) -> None:
            if isinstance(content, str) and content == "ready":
                widget._on_ready()

        self.on_msg(handle_msg)

    def _on_ready(self) -> None:
        self._ready = True
        for rrd in self._rrd_buffer:
            self.send_rrd(rrd)
        self._rrd_buffer.clear()

    def send_rrd(self, data: bytes) -> None:
        """Send a recording to the viewer."""

        if not self._ready:
            self._rrd_buffer.append(data)
            return

        self.send({"type": "rrd"}, buffers=[data])
