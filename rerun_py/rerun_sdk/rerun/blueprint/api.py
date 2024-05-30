from __future__ import annotations

import uuid
from typing import Any, Iterable, Optional, Union

import rerun_bindings as bindings

from .._baseclasses import AsComponents
from .._spawn import _spawn_viewer
from ..datatypes import EntityPathLike, Utf8ArrayLike, Utf8Like
from ..memory import MemoryRecording
from ..notebook import as_html
from ..recording_stream import RecordingStream
from .archetypes import ContainerBlueprint, PanelBlueprint, SpaceViewBlueprint, SpaceViewContents, ViewportBlueprint
from .components import ColumnShareArrayLike, PanelState, PanelStateLike, RowShareArrayLike, VisibleLike
from .components.container_kind import ContainerKindLike

SpaceViewContentsLike = Union[Utf8ArrayLike, SpaceViewContents]


class SpaceView:
    """
    Base class for all space view types.

    Consider using one of the subclasses instead of this class directly:

    - [rerun.blueprint.BarChartView][]
    - [rerun.blueprint.Spatial2DView][]
    - [rerun.blueprint.Spatial3DView][]
    - [rerun.blueprint.TensorView][]
    - [rerun.blueprint.TextDocumentView][]
    - [rerun.blueprint.TextLogView][]
    - [rerun.blueprint.TimeSeriesView][]

    These are ergonomic helpers on top of [rerun.blueprint.archetypes.SpaceViewBlueprint][].
    """

    def __init__(
        self,
        *,
        class_identifier: Utf8Like,
        origin: EntityPathLike,
        contents: SpaceViewContentsLike,
        name: Utf8Like | None,
        visible: VisibleLike | None = None,
        properties: dict[str, AsComponents] = {},
    ):
        """
        Construct a blueprint for a new space view.

        Parameters
        ----------
        name
            The name of the space view.
        class_identifier
            The class of the space view to add. This must correspond to a known space view class.
            Prefer to use one of the subclasses of `SpaceView` which will populate this for you.
        origin
            The `EntityPath` to use as the origin of this space view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the space view specified as a query expression. This is either a single expression,
            or a list of multiple expressions. See [rerun.blueprint.archetypes.SpaceViewContents][].
        visible:
            Whether this space view is visible.

            Defaults to true if not specified.
        properties
            Dictionary of property archetypes to add to space view's internal hierarchy.

        """
        self.id = uuid.uuid4()
        self.class_identifier = class_identifier
        self.name = name
        self.origin = origin
        self.contents = contents
        self.visible = visible
        self.properties = properties

    def blueprint_path(self) -> str:
        """
        The blueprint path where this space view will be logged.

        Note that although this is an `EntityPath`, is scoped to the blueprint tree and
        not a part of the regular data hierarchy.
        """
        return f"space_view/{self.id}"

    def to_container(self) -> Container:
        """Convert this space view to a container."""
        from .containers import Tabs

        return Tabs(self)

    def to_blueprint(self) -> Blueprint:
        """Convert this space view to a full blueprint."""
        return Blueprint(self)

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        if isinstance(self.contents, SpaceViewContents):
            # If contents is already a SpaceViewContents, we can just use it directly
            contents = self.contents
        else:
            # Otherwise we delegate to the SpaceViewContents constructor
            contents = SpaceViewContents(query=self.contents)  # type: ignore[arg-type]

        stream.log(self.blueprint_path() + "/SpaceViewContents", contents)  # type: ignore[attr-defined]

        arch = SpaceViewBlueprint(
            class_identifier=self.class_identifier,
            display_name=self.name,
            space_origin=self.origin,
            visible=self.visible,
        )

        stream.log(self.blueprint_path(), arch, recording=stream)  # type: ignore[attr-defined]

        for prop_name, prop in self.properties.items():
            stream.log(f"{self.blueprint_path()}/{prop_name}", prop, recording=stream)  # type: ignore[attr-defined]

    def _repr_html_(self) -> Any:
        """IPython interface to conversion to html."""
        return as_html(blueprint=self)


class Container:
    """
    Base class for all container types.

    Consider using one of the subclasses instead of this class directly:

    - [rerun.blueprint.Horizontal][]
    - [rerun.blueprint.Vertical][]
    - [rerun.blueprint.Grid][]
    - [rerun.blueprint.Tabs][]

    These are ergonomic helpers on top of [rerun.blueprint.archetypes.ContainerBlueprint][].
    """

    def __init__(
        self,
        *args: Container | SpaceView,
        contents: Optional[Iterable[Container | SpaceView]] = None,
        kind: ContainerKindLike,
        column_shares: Optional[ColumnShareArrayLike] = None,
        row_shares: Optional[RowShareArrayLike] = None,
        grid_columns: Optional[int] = None,
        active_tab: Optional[int | str] = None,
        name: Utf8Like | None,
    ):
        """
        Construct a new container.

        Parameters
        ----------
        *args:
            All positional arguments are forwarded to the `contents` parameter for convenience.
        contents:
            The contents of the container. Each item in the iterable must be a `SpaceView` or a `Container`.
            This can only be used if no positional arguments are provided.
        kind
            The kind of the container. This must correspond to a known container kind.
            Prefer to use one of the subclasses of `Container` which will populate this for you.
        column_shares
            The layout shares of the columns in the container. The share is used to determine what fraction of the total width each
            column should take up. The column with index `i` will take up the fraction `shares[i] / total_shares`.
            This is only applicable to `Horizontal` or `Grid` containers.
        row_shares
            The layout shares of the rows in the container. The share is used to determine what fraction of the total height each
            row should take up. The row with index `i` will take up the fraction `shares[i] / total_shares`.
            This is only applicable to `Vertical` or `Grid` containers.
        grid_columns
            The number of columns in the grid. This is only applicable to `Grid` containers.
        active_tab
            The active tab in the container. This is only applicable to `Tabs` containers.
        name
            The name of the container

        """

        if args and contents is not None:
            raise ValueError("Cannot provide both positional and keyword arguments for contents")

        if contents is not None:
            self.contents = contents
        else:
            self.contents = args

        self.id = uuid.uuid4()
        self.kind = kind
        self.column_shares = column_shares
        self.row_shares = row_shares
        self.grid_columns = grid_columns
        self.active_tab = active_tab
        self.name = name

    def blueprint_path(self) -> str:
        """
        The blueprint path where this space view will be logged.

        Note that although this is an `EntityPath`, is scoped to the blueprint tree and
        not a part of the regular data hierarchy.
        """
        return f"container/{self.id}"

    def to_container(self) -> Container:
        """Convert this space view to a container."""
        return self

    def to_blueprint(self) -> Blueprint:
        """Convert this container to a full blueprint."""
        return Blueprint(self)

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        active_tab_path = None

        for i, sub in enumerate(self.contents):
            sub._log_to_stream(stream)
            if i == self.active_tab or (isinstance(sub, SpaceView) and sub.name == self.active_tab):
                active_tab_path = sub.blueprint_path()

        if self.active_tab is not None and active_tab_path is None:
            raise ValueError(f"Active tab '{self.active_tab}' not found in the container contents.")

        arch = ContainerBlueprint(
            container_kind=self.kind,
            contents=[sub.blueprint_path() for sub in self.contents],
            col_shares=self.column_shares,
            row_shares=self.row_shares,
            visible=True,
            grid_columns=self.grid_columns,
            active_tab=active_tab_path,
            display_name=self.name,
        )

        stream.log(self.blueprint_path(), arch)  # type: ignore[attr-defined]

    def _repr_html_(self) -> Any:
        """IPython interface to conversion to html."""
        return as_html(blueprint=self)


def _to_state(expanded: bool | None, state: PanelStateLike | None) -> PanelStateLike | None:
    """Handle the case where `expanded` is used over `state`."""

    if expanded is not None and state is not None:
        raise ValueError("Cannot set both 'expanded' and 'state'")
    if state is not None:
        return state
    if expanded is not None:
        return PanelState.Expanded if expanded else PanelState.Collapsed
    return None


class Panel:
    """
    Base class for the panel types.

    Consider using one of the subclasses instead of this class directly:

    - [TopPanel][rerun.blueprint.TopPanel]
    - [BlueprintPanel][rerun.blueprint.BlueprintPanel]
    - [SelectionPanel][rerun.blueprint.SelectionPanel]
    - [TimePanel][rerun.blueprint.TimePanel]

    These are ergonomic helpers on top of [rerun.blueprint.archetypes.PanelBlueprint][].
    """

    def __init__(self, *, blueprint_path: str, expanded: bool | None = None, state: PanelStateLike | None = None):
        """
        Construct a new panel.

        Parameters
        ----------
        blueprint_path:
            The blueprint path of the panel.
        expanded:
            Deprecated. Use `state` instead.
        state:
            Whether the panel is expanded, collapsed, or hidden.

        """
        self._blueprint_path = blueprint_path
        self.state = _to_state(expanded, state)

    def blueprint_path(self) -> str:
        """
        The blueprint path where this space view will be logged.

        Note that although this is an `EntityPath`, is scoped to the blueprint tree and
        not a part of the regular data hierarchy.
        """
        return self._blueprint_path

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        arch = PanelBlueprint(
            state=self.state,
        )

        stream.log(self.blueprint_path(), arch)  # type: ignore[attr-defined]


class TopPanel(Panel):
    """The state of the top panel."""

    def __init__(self, *, expanded: bool | None = None, state: PanelStateLike | None = None):
        """
        Construct a new top panel.

        Parameters
        ----------
        expanded:
            Deprecated. Use `state` instead.
        state:
            Whether the panel is expanded, collapsed, or hidden.

            Collapsed and hidden both fully hide the top panel.

        """
        super().__init__(blueprint_path="top_panel", expanded=expanded, state=state)


class BlueprintPanel(Panel):
    """The state of the blueprint panel."""

    def __init__(self, *, expanded: bool | None = None, state: PanelStateLike | None = None):
        """
        Construct a new blueprint panel.

        Parameters
        ----------
        expanded:
            Deprecated. Use `state` instead.
        state:
            Whether the panel is expanded, collapsed, or hidden.

            Collapsed and hidden both fully hide the blueprint panel.

        """
        super().__init__(blueprint_path="blueprint_panel", expanded=expanded, state=state)


class SelectionPanel(Panel):
    """The state of the selection panel."""

    def __init__(self, *, expanded: bool | None = None, state: PanelStateLike | None = None):
        """
        Construct a new selection panel.

        Parameters
        ----------
        expanded:
            Deprecated. Use `state` instead.
        state:
            Whether the panel is expanded, collapsed, or hidden.

            Collapsed and hidden both fully hide the selection panel.

        """
        super().__init__(blueprint_path="selection_panel", expanded=expanded, state=state)


class TimePanel(Panel):
    """The state of the time panel."""

    def __init__(self, *, expanded: bool | None = None, state: PanelStateLike | None = None):
        """
        Construct a new time panel.

        Parameters
        ----------
        expanded:
            Deprecated. Use `state` instead.
        state:
            Whether the panel is expanded, collapsed, or hidden.

            Expanded fully shows the panel, collapsed shows a simplified panel,
            hidden fully hides the panel.

        """
        super().__init__(blueprint_path="time_panel", expanded=expanded, state=state)


ContainerLike = Union[Container, SpaceView]
"""
A type that can be converted to a container.

These types all implement a `to_container()` method that wraps them in the necessary
helper classes.
"""

BlueprintPart = Union[ContainerLike, TopPanel, BlueprintPanel, SelectionPanel, TimePanel]
"""
The types that make up a blueprint.
"""


class Blueprint:
    """The top-level description of the viewer blueprint."""

    def __init__(
        self,
        *parts: BlueprintPart,
        auto_layout: bool | None = None,
        auto_space_views: bool | None = None,
        collapse_panels: bool = False,
    ):
        """
        Construct a new blueprint from the given parts.

        Each [BlueprintPart][rerun.blueprint.BlueprintPart] can be one of the following:

        - [ContainerLike][rerun.blueprint.ContainerLike]
        - [BlueprintPanel][rerun.blueprint.BlueprintPanel]
        - [SelectionPanel][rerun.blueprint.SelectionPanel]
        - [TimePanel][rerun.blueprint.TimePanel]

        It is an error to provide more than one of instance of any of the panel types.

        Blueprints only have a single top-level "root" container that defines the viewport.
        If you provide multiple `ContainerLike` instances, they will be combined under a single
        root `Tab` container.

        Parameters
        ----------
        *parts:
            The parts of the blueprint.
        auto_layout:
            Whether to automatically layout the viewport. If `True`, the container layout will be
            reset whenever a new space view is added to the viewport. Defaults to `False`.
            Defaults to `False` unless no Containers or SpaceViews are provided, in which case it defaults to `True`.
            If you want to create a completely empty Blueprint, you must explicitly set this to `False`.
        auto_space_views:
            Whether to automatically add space views to the viewport. If `True`, the viewport will
            automatically add space views based on content in the data store.
            Defaults to `False` unless no Containers or SpaceViews are provided, in which case it defaults to `True`.
            If you want to create a completely empty Blueprint, you must explicitly set this to `False`.
        collapse_panels:
            Whether to collapse panels in the viewer. Defaults to `False`.

            This fully hides the blueprint/selection panels, and shows the simplified time panel.

        """
        from .containers import Tabs

        self.collapse_panels = collapse_panels

        contents: list[ContainerLike] = []

        for part in parts:
            if isinstance(part, (Container, SpaceView)):
                contents.append(part)
            elif isinstance(part, TopPanel):
                if hasattr(self, "top_panel"):
                    raise ValueError("Only one top panel can be provided")
                self.top_panel = part
            elif isinstance(part, BlueprintPanel):
                if hasattr(self, "blueprint_panel"):
                    raise ValueError("Only one blueprint panel can be provided")
                self.blueprint_panel = part
            elif isinstance(part, SelectionPanel):
                if hasattr(self, "selection_panel"):
                    raise ValueError("Only one selection panel can be provided")
                self.selection_panel = part
            elif isinstance(part, TimePanel):
                if hasattr(self, "time_panel"):
                    raise ValueError("Only one time panel can be provided")
                self.time_panel = part
            else:
                raise ValueError(f"Unknown part type: {part}")

        self.auto_space_views = auto_space_views
        self.auto_layout = auto_layout

        if len(contents) == 0:
            # If there's no content, switch `auto_layout` and `auto_space_views` defaults to `True`.
            if self.auto_space_views is None:
                self.auto_space_views = True
            if self.auto_layout is None:
                self.auto_layout = True
        elif len(contents) == 1:
            self.root_container = contents[0].to_container()
        else:
            self.root_container = Tabs(contents=contents)

    def to_blueprint(self) -> Blueprint:
        """Conform with the `BlueprintLike` interface."""
        return self

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        if hasattr(self, "root_container"):
            self.root_container._log_to_stream(stream)

            root_container_id = self.root_container.id.bytes
        else:
            root_container_id = None

        viewport_arch = ViewportBlueprint(
            root_container=root_container_id,
            auto_layout=self.auto_layout,
            auto_space_views=self.auto_space_views,
        )

        stream.log("viewport", viewport_arch)  # type: ignore[attr-defined]

        if hasattr(self, "top_panel"):
            self.top_panel._log_to_stream(stream)

        if hasattr(self, "blueprint_panel"):
            self.blueprint_panel._log_to_stream(stream)
        elif self.collapse_panels:
            BlueprintPanel(state="collapsed")._log_to_stream(stream)

        if hasattr(self, "selection_panel"):
            self.selection_panel._log_to_stream(stream)
        elif self.collapse_panels:
            SelectionPanel(state="collapsed")._log_to_stream(stream)

        if hasattr(self, "time_panel"):
            self.time_panel._log_to_stream(stream)
        elif self.collapse_panels:
            TimePanel(state="collapsed")._log_to_stream(stream)

    def _repr_html_(self) -> Any:
        """IPython interface to conversion to html."""
        return as_html(blueprint=self)

    def connect(
        self,
        application_id: str,
        *,
        addr: str | None = None,
        make_active: bool = True,
        make_default: bool = True,
    ) -> None:
        """
        Connect to a remote Rerun Viewer on the given ip:port and send this blueprint.

        Parameters
        ----------
        application_id:
            The application ID to use for this blueprint. This must match the application ID used
            when initiating rerun for any data logging you wish to associate with this blueprint.
        addr:
            The ip:port to connect to
        make_active:
            Immediately make this the active blueprint for the associated `app_id`.
            Note that setting this to `false` does not mean the blueprint may not still end
            up becoming active. In particular, if `make_default` is true and there is no other
            currently active blueprint.
        make_default:
            Make this the default blueprint for the `app_id`.
            The default blueprint will be used as the template when the user resets the
            blueprint for the app. It will also become the active blueprint if no other
            blueprint is currently active.

        """
        blueprint_stream = RecordingStream(
            bindings.new_blueprint(
                application_id=application_id,
                make_default=False,
                make_thread_default=False,
                default_enabled=True,
            )
        )
        blueprint_stream.set_time_sequence("blueprint", 0)  # type: ignore[attr-defined]
        self._log_to_stream(blueprint_stream)

        bindings.connect_blueprint(addr, make_active, make_default, blueprint_stream.to_native())

    def save(self, application_id: str, path: str | None = None) -> None:
        """
        Save this blueprint to a file. Rerun recommends the `.rbl` suffix.

        Parameters
        ----------
        application_id:
            The application ID to use for this blueprint. This must match the application ID used
            when initiating rerun for any data logging you wish to associate with this blueprint.
        path
            The path to save the blueprint to. Defaults to `<application_id>.rbl`.

        """

        if path is None:
            path = f"{application_id}.rbl"

        blueprint_stream = RecordingStream(
            bindings.new_blueprint(
                application_id=application_id,
                make_default=False,
                make_thread_default=False,
                default_enabled=True,
            )
        )
        blueprint_stream.set_time_sequence("blueprint", 0)  # type: ignore[attr-defined]
        self._log_to_stream(blueprint_stream)

        bindings.save_blueprint(path, blueprint_stream.to_native())

    def spawn(
        self, application_id: str, port: int = 9876, memory_limit: str = "75%", hide_welcome_screen: bool = False
    ) -> None:
        """
        Spawn a Rerun viewer with this blueprint.

        Parameters
        ----------
        application_id:
            The application ID to use for this blueprint. This must match the application ID used
            when initiating rerun for any data logging you wish to associate with this blueprint.
        port:
            The port to listen on.
        memory_limit:
            An upper limit on how much memory the Rerun Viewer should use.
            When this limit is reached, Rerun will drop the oldest data.
            Example: `16GB` or `50%` (of system total).
        hide_welcome_screen:
            Hide the normal Rerun welcome screen.

        """
        _spawn_viewer(port=port, memory_limit=memory_limit, hide_welcome_screen=hide_welcome_screen)
        self.connect(application_id=application_id, addr=f"127.0.0.1:{port}")


BlueprintLike = Union[Blueprint, SpaceView, Container]
"""
A type that can be converted to a blueprint.

These types all implement a `to_blueprint()` method that wraps them in the necessary
helper classes.
"""


def create_in_memory_blueprint(*, application_id: str, blueprint: BlueprintLike) -> MemoryRecording:
    """Internal rerun helper to convert a `BlueprintLike` into a stream that can be sent to the viewer."""

    # Convert the BlueprintLike to a full blueprint
    blueprint = blueprint.to_blueprint()

    # We only use this stream object directly, so don't need to make it
    # default or thread default. Making it the thread-default will also
    # lead to an unnecessary warning on mac/win.
    blueprint_stream = RecordingStream(
        bindings.new_blueprint(
            application_id=application_id,
            make_default=False,
            make_thread_default=False,
            default_enabled=True,
        )
    )

    blueprint_stream.set_time_sequence("blueprint", 0)  # type: ignore[attr-defined]

    blueprint._log_to_stream(blueprint_stream)

    return blueprint_stream.memory_recording()  # type: ignore[attr-defined, no-any-return]
