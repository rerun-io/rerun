from __future__ import annotations

import importlib.metadata
import logging
import os
import pathlib
import time
from collections.abc import Mapping
from dataclasses import dataclass
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
    ESM_MOD: str | pathlib.Path = f"http://localhost:{bound_addr[1]}/widget.js"
elif ASSET_ENV == ASSET_MAGIC_INLINE:
    ESM_MOD = WIDGET_PATH
else:
    ESM_MOD = ASSET_ENV
    if not (ASSET_ENV.startswith(("http://", "https://"))):
        raise ValueError(f"RERUN_NOTEBOOK_ASSET should be a URL starting with http or https. Found: {ASSET_ENV}")


# If you add a callback here, you should also update the `callbacks.ipynb` notebook to showcase it.
class ViewerCallbacks:
    """
    Base class for Viewer callback definitions.

    You should inherit from this class, override the callback methods you need,
    and then pass an instance to `Viewer.register_callbacks`.
    """

    def on_selection_change(self, selection: list[SelectionItem]) -> None:
        """
        Fired when the selection changes.

        Examples:
        * Clicking on an entity
        * Clicking on an entity instance
        * Clicking on or inside a view
        * Clicking on a container in the left panel

        This event is fired each time any part of the event payload changes,
        this includes for example clicking on different parts of the same
        entity in a 2D or 3D view.
        """

    def on_timeline_change(self, timeline: str, time: float) -> None:
        """Fired when a different timeline is selected."""

    def on_time_update(self, time: float) -> None:
        """Fired when the timepoint changes."""


@dataclass
class EntitySelection:
    """
    Selected an entity, or an instance of an entity.

    If the entity was selected within a view, then this also
    includes the `view_id`.

    If the entity was selected within a 2D or 3D space view,
    then this also includes the position.
    """

    @property
    def kind(self) -> Literal["entity"]:
        return "entity"

    entity_path: str
    instance_id: int | None
    view_id: str | None
    position: tuple[int, int, int] | None


@dataclass
class ViewSelection:
    """Selected a view."""

    @property
    def kind(self) -> Literal["view"]:
        return "view"

    view_id: str


@dataclass
class ContainerSelection:
    """Selected a container."""

    @property
    def kind(self) -> Literal["container"]:
        return "container"

    container_id: str


SelectionItem = EntitySelection | ViewSelection | ContainerSelection
"""A single item in a selection."""


def _selection_item_from_json(json: Any) -> SelectionItem:
    if json["type"] == "entity":
        position = json.get("position", None)
        return EntitySelection(
            entity_path=json["entity_path"],
            instance_id=json.get("instance_id", None),
            view_id=json.get("view_id", None),
            position=(position[0], position[1], position[2]) if position is not None else None,
        )
    if json["type"] == "view":
        return ViewSelection(view_id=json["view_id"])
    if json["type"] == "container":
        return ContainerSelection(container_id=json["container_id"])
    else:
        raise NotImplementedError(f"selection item kind {json[type]} is not handled")


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

    _time_ctrl = traitlets.Tuple(
        traitlets.Unicode(allow_none=True),
        traitlets.Int(allow_none=True),
        traitlets.Bool(),
        allow_none=True,
    ).tag(sync=True)
    _recording_id = traitlets.Unicode(allow_none=True).tag(sync=True)

    _callbacks: list[ViewerCallbacks] = []

    def __init__(
        self,
        *,
        width: int | None = None,
        height: int | None = None,
        url: str | None = None,
        panel_states: Mapping[Panel, PanelState] | None = None,
        **kwargs: Any,
    ) -> None:
        super().__init__(**kwargs)

        self._width = width
        self._height = height
        self._url = url
        self._data_queue = []

        from ipywidgets import widgets

        self._output = widgets.Output()

        if panel_states:
            self.update_panel_states(panel_states)

        def handle_msg(widget: Any, content: Any, buffers: list[bytes]) -> None:
            if isinstance(content, str) and content == "ready":
                self._on_ready()
            elif not isinstance(content, str) and "event" in content:
                # Event names here come from `widget.ts`.
                if content["event"] == "selectionchange":
                    selection = [_selection_item_from_json(item) for item in content["payload"]]
                    for callback in self._callbacks:
                        callback.on_selection_change(selection)
                elif content["event"] == "timelinechange":
                    timeline = content["payload"]["timeline"]
                    time = content["payload"]["time"]
                    for callback in self._callbacks:
                        callback.on_timeline_change(timeline, time)
                elif content["event"] == "timeupdate":
                    time = content["payload"]
                    for callback in self._callbacks:
                        callback.on_time_update(time)

        self.on_msg(handle_msg)

    def _on_ready(self) -> None:
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

    def block_until_ready(self, timeout: float = 5.0) -> None:
        """Block until the viewer is ready."""

        start = time.time()

        with jupyter_ui_poll.ui_events() as poll:
            while self._ready is False:
                if time.time() - start > timeout:
                    logging.warning(
                        f"""Timed out waiting for viewer to become ready. Make sure: {ESM_MOD} is accessible.
If not, consider setting `RERUN_NOTEBOOK_ASSET`. Consult https://pypi.org/project/rerun-notebook/{__version__}/ for details.
""",
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

    def register_callbacks(self, callbacks: ViewerCallbacks) -> None:
        """Register a set of callbacks with this instance of the Viewer."""
        # TODO(jan): maybe allow unregister by making this a map instead
        self._callbacks.append(callbacks)
