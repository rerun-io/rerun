from __future__ import annotations

import itertools
import uuid
from typing import Any, Iterable, Optional, Union

import rerun_bindings as bindings

from ..datatypes import EntityPathLike, Utf8ArrayLike, Utf8Like
from ..memory import MemoryRecording, memory_recording
from ..recording_stream import RecordingStream, get_application_id
from .archetypes import ContainerBlueprint, PanelBlueprint, SpaceViewBlueprint, SpaceViewContents, ViewportBlueprint
from .components import ColumnShareArrayLike, RowShareArrayLike
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

        """
        self.id = uuid.uuid4()
        self.class_identifier = class_identifier
        self.name = name
        self.origin = origin
        self.contents = contents

    def blueprint_path(self) -> str:
        """
        The blueprint path where this space view will be logged.

        Note that although this is an `EntityPath`, is scoped to the blueprint tree and
        not a part of the regular data hierarchy.
        """
        return f"space_view/{self.id}"

    def to_container(self) -> Container:
        """Convert this space view to a container."""
        from .containers import Grid

        return Grid(self)

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
        )

        stream.log(self.blueprint_path(), arch, recording=stream)  # type: ignore[attr-defined]

    def _iter_space_views(self) -> Iterable[bytes]:
        """Internal method to iterate over all of the space views in the blueprint."""
        # TODO(jleibs): This goes away when we get rid of `space_views` from the viewport and just use
        # the entity-path lookup instead.
        return [self.id.bytes]


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

    def _iter_space_views(self) -> Iterable[bytes]:
        """Internal method to iterate over all of the space views in the blueprint."""
        # TODO(jleibs): This goes away when we get rid of `space_views` from the viewport and just use
        # the entity-path lookup instead.
        return itertools.chain.from_iterable(sub._iter_space_views() for sub in self.contents)


class Panel:
    """
    Base class for the panel types.

    Consider using one of the subclasses instead of this class directly:

    - [BlueprintPanel][rerun.blueprint.BlueprintPanel]
    - [SelectionPanel][rerun.blueprint.SelectionPanel]
    - [TimePanel][rerun.blueprint.TimePanel]

    These are ergonomic helpers on top of [rerun.blueprint.archetypes.PanelBlueprint][].
    """

    def __init__(self, *, blueprint_path: str, expanded: bool | None = None):
        """
        Construct a new panel.

        Parameters
        ----------
        blueprint_path:
            The blueprint path of the panel.
        expanded:
            Whether the panel is expanded or not.

        """
        self._blueprint_path = blueprint_path
        self.expanded = expanded

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
            expanded=self.expanded,
        )

        stream.log(self.blueprint_path(), arch)  # type: ignore[attr-defined]


class BlueprintPanel(Panel):
    """The state of the blueprint panel."""

    def __init__(self, *, expanded: bool | None = None):
        """
        Construct a new blueprint panel.

        Parameters
        ----------
        expanded:
            Whether the panel is expanded or not.

        """
        super().__init__(blueprint_path="blueprint_panel", expanded=expanded)


class SelectionPanel(Panel):
    """The state of the selection panel."""

    def __init__(self, *, expanded: bool | None = None):
        """
        Construct a new selection panel.

        Parameters
        ----------
        expanded:
            Whether the panel is expanded or not.

        """
        super().__init__(blueprint_path="selection_panel", expanded=expanded)


class TimePanel(Panel):
    """The state of the time panel."""

    def __init__(self, *, expanded: bool | None = None):
        """
        Construct a new time panel.

        Parameters
        ----------
        expanded:
            Whether the panel is expanded or not.

        """
        super().__init__(blueprint_path="time_panel", expanded=expanded)


ContainerLike = Union[Container, SpaceView]
"""
A type that can be converted to a container.

These types all implement a `to_container()` method that wraps them in the necessary
helper classes.
"""

BlueprintPart = Union[ContainerLike, BlueprintPanel, SelectionPanel, TimePanel]
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
    ):
        """
        Construct a new blueprint from the given parts.

        Each [BlueprintPart][rerun.blueprint.BlueprintPart] can be one of the following:

        - [ContainerLike][rerun.blueprint.ContainerLike]
        - [BlueprintPanel][rerun.blueprint.BlueprintPanel]
        - [SelectionPanel][rerun.blueprint.SelectionPanel]
        - [TimePanel][rerun.blueprint.TimePanel]

        It is an error to provide more than one of any type of part.

        Blueprints only have a single top-level "root" container that defines the viewport. Any
        other content should be nested under this container (or a nested sub-container).

        Parameters
        ----------
        *parts:
            The parts of the blueprint.
        auto_layout:
            Whether to automatically layout the viewport. If `True`, the container layout will be
            reset whenever a new space view is added to the viewport. Defaults to `False`.
        auto_space_views:
            Whether to automatically add space views to the viewport. If `True`, the viewport will
            automatically add space views based on content in the data store. Defaults to `False`.

        """

        for part in parts:
            if isinstance(part, (Container, SpaceView)):
                if hasattr(self, "root_container"):
                    raise ValueError(
                        "Only one ContainerLike can be provided to serve as the root container for the viewport"
                    )
                self.root_container = part.to_container()
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

        self.auto_layout = auto_layout
        self.auto_space_views = auto_space_views

    def to_blueprint(self) -> Blueprint:
        """Conform with the `BlueprintLike` interface."""
        return self

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        if hasattr(self, "root_container"):
            self.root_container._log_to_stream(stream)

            root_container_id = self.root_container.id.bytes
            space_views = list(self.root_container._iter_space_views())
        else:
            root_container_id = None
            space_views = []

        viewport_arch = ViewportBlueprint(
            space_views=space_views,
            root_container=root_container_id,
            auto_layout=self.auto_layout,
            auto_space_views=self.auto_space_views,
        )

        stream.log("viewport", viewport_arch)  # type: ignore[attr-defined]

        if hasattr(self, "blueprint_panel"):
            self.blueprint_panel._log_to_stream(stream)
        if hasattr(self, "selection_panel"):
            self.selection_panel._log_to_stream(stream)
        if hasattr(self, "time_panel"):
            self.time_panel._log_to_stream(stream)

    def as_html(self, data_stream: RecordingStream | None = None) -> Any:
        application_id = get_application_id(recording=data_stream)

        # TODO(jleibs): Too many hoops. Some refactoring here would simplify this a lot
        final_stream = RecordingStream(
            bindings.new_recording(
                application_id=application_id,
                make_default=False,
                make_thread_default=False,
                default_enabled=True,
            )
        )
        final_stream.send_blueprint(self)  # type: ignore[attr-defined]
        data_memory = memory_recording(recording=data_stream)

        final_memory = final_stream.memory_recording()  # type: ignore[attr-defined]
        return final_memory.as_html(other=data_memory)

    def _repr_html_(self) -> Any:
        return self.as_html()


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
