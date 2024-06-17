from __future__ import annotations

import importlib.metadata
import pathlib
from typing import Literal

import anywidget
import traitlets

try:
    __version__ = importlib.metadata.version("rerun_notebook")
except importlib.metadata.PackageNotFoundError:
    __version__ = "unknown"


Panel = Literal["top", "blueprint", "selection", "time"]
PanelState = Literal["expanded", "collapsed", "hidden"]


# read JS/CSS at import time
# this slows down the import slightly, but it's better than reading the files every time a widget is created
# _esm = (pathlib.Path(__file__).parent / "static" / "widget.js").read_text()
# _css = (pathlib.Path(__file__).parent / "static" / "widget.css").read_text()


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

    _data = traitlets.Bytes(allow_none=True).tag(sync=True)

    def __init__(
        self,
        *,
        width: int | None = None,
        height: int | None = None,
        url: str | None = None,
        panel_states: dict[Panel, PanelState] | None = None,
        recording: bytes | None = None,
        **kwargs,
    ):
        super().__init__(**kwargs)

        self._width = width
        self._height = height
        self._url = url
        self._panel_states = panel_states
        self._data = recording

        """ def handle_msg(widget: Viewer, content: Any, buffers: list[bytes]) -> None:
            if isinstance(content, str) and content == "ready":
                widget._on_ready()

        self.on_msg(handle_msg) """

    def send_rrd(self, data: bytes) -> None:
        """Send a recording to the viewer."""

        self._data = data
