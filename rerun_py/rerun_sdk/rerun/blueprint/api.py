from __future__ import annotations

import itertools
import uuid
from typing import Iterable, Optional, Union

import rerun_bindings as bindings

from ..datatypes import EntityPathLike, Utf8ArrayLike, Utf8Like
from ..recording import MemoryRecording
from ..recording_stream import RecordingStream
from .archetypes import ContainerBlueprint, PanelBlueprint, SpaceViewBlueprint, SpaceViewContents, ViewportBlueprint
from .components import ColumnShareArrayLike, RowShareArrayLike
from .components.container_kind import ContainerKindLike

SpaceViewContentsLike = Union[Utf8ArrayLike, SpaceViewContents]


class SpaceView:
    """
    Base class for all space view types.

    Consider using one of the subclasses instead of this class directly:
    - [Spatial3D][] for 3D space views
    - [Spatial2D][] for 2D space views

    This is an ergonomic helper on top of [rerun.blueprint.archetypes.SpaceViewBlueprint][].
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
            The contents of the space view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.

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

    def to_viewport(self) -> Viewport:
        """Convert this space view to a viewport."""
        from .containers import Grid

        return Viewport(Grid(self))

    def to_blueprint(self) -> Blueprint:
        """Convert this space view to a full blueprint."""
        return Blueprint(self.to_viewport())

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
    - [Horizontal][] for horizontal containers
    - [Vertical][] for vertical containers
    - [Grid][] for grid containers
    - [Tabs][] for tab containers

    This is an ergonomic helper on top of [rerun.blueprint.archetypes.ContainerBlueprint][].
    """

    def __init__(
        self,
        *contents: Container | SpaceView,
        kind: ContainerKindLike,
        column_shares: Optional[ColumnShareArrayLike] = None,
        row_shares: Optional[RowShareArrayLike] = None,
        grid_columns: Optional[int] = None,
        active_tab: Optional[int | str] = None,
    ):
        """
        Construct a new container.

        Parameters
        ----------
        *contents:
            All positional arguments are the contents of the container, which may be either other containers or space views.
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

        """
        self.id = uuid.uuid4()
        self.kind = kind
        self.contents = contents
        self.column_shares = column_shares
        self.row_shares = row_shares
        self.grid_columns = grid_columns
        self.active_tab = active_tab

    def blueprint_path(self) -> str:
        """
        The blueprint path where this space view will be logged.

        Note that although this is an `EntityPath`, is scoped to the blueprint tree and
        not a part of the regular data hierarchy.
        """
        return f"container/{self.id}"

    def to_viewport(self) -> Viewport:
        """Convert this container to a viewport."""
        return Viewport(self)

    def to_blueprint(self) -> Blueprint:
        """Convert this container to a full blueprint."""
        return Blueprint(self.to_viewport())

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
        )

        stream.log(self.blueprint_path(), arch)  # type: ignore[attr-defined]

    def _iter_space_views(self) -> Iterable[bytes]:
        """Internal method to iterate over all of the space views in the blueprint."""
        # TODO(jleibs): This goes away when we get rid of `space_views` from the viewport and just use
        # the entity-path lookup instead.
        return itertools.chain.from_iterable(sub._iter_space_views() for sub in self.contents)


class Viewport:
    """
    The top-level description of the Viewport.

    This is an ergonomic helper on top of [rerun.blueprint.archetypes.ViewportBlueprint][].
    """

    def __init__(
        self, root_container: Container, *, auto_layout: bool | None = None, auto_space_views: bool | None = None
    ):
        """
        Construct a new viewport.

        Parameters
        ----------
        root_container:
            The container that sits at the top of the viewport hierarchy. The only content visible
            in this viewport must be contained within this container.
        auto_layout:
            Whether to automatically layout the viewport. If `True`, the container layout will be
            reset whenever a new space view is added to the viewport. Defaults to `False`.
        auto_space_views:
            Whether to automatically add space views to the viewport. If `True`, the viewport will
            automatically add space views based on content in the data store. Defaults to `False`.

        """
        self.root_container = root_container
        self.auto_layout = auto_layout
        self.auto_space_views = auto_space_views

    def blueprint_path(self) -> str:
        """
        The blueprint path where this space view will be logged.

        Note that although this is an `EntityPath`, is scoped to the blueprint tree and
        not a part of the regular data hierarchy.
        """
        return "viewport"

    def to_viewport(self) -> Viewport:
        """Conform with the `ViewportLike` interface."""
        return self

    def to_blueprint(self) -> Blueprint:
        """Convert this viewport to a full blueprint."""
        return Blueprint(self)

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        self.root_container._log_to_stream(stream)

        arch = ViewportBlueprint(
            space_views=list(self.root_container._iter_space_views()),
            root_container=self.root_container.id.bytes,
            auto_layout=self.auto_layout,
            auto_space_views=self.auto_space_views,
        )

        stream.log(self.blueprint_path(), arch)  # type: ignore[attr-defined]


class Panel:
    """
    Base class for the panel types.

    Consider using one of the subclasses instead of this class directly:
    - [BlueprintPanel][]
    - [SelectionPanel][]
    - [TimePanel][]

    This is an ergonomic helper on top of [rerun.blueprint.archetypes.PanelBlueprint][].
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


ViewportLike = Union[Viewport, Container, SpaceView]
"""
A type that can be converted to a viewport.

These types all implement a `to_viewport()` method that wraps them in the necessary
helper classes.
"""

BlueprintPart = Union[ViewportLike, BlueprintPanel, SelectionPanel, TimePanel]
"""
The types that make up a blueprint.
"""


class Blueprint:
    """The top-level description of the viewer blueprint."""

    def __init__(
        self,
        *parts: BlueprintPart,
    ):
        """
        Construct a new blueprint from the given parts.

        Each [BlueprintPart][] can be one of the following:
        - [Viewport][]
        - [BlueprintPanel][]
        - [SelectionPanel][]
        - [TimePanel][]

        It is an error to provide more than one of any type of part.

        Parameters
        ----------
        *parts:
            The parts of the blueprint.

        """

        for part in parts:
            if isinstance(part, (Viewport, Container, SpaceView)):
                if hasattr(self, "viewport"):
                    raise ValueError("Only one viewport can be provided")
                self.viewport = part.to_viewport()
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

    def to_blueprint(self) -> Blueprint:
        """Conform with the `BlueprintLike` interface."""
        return self

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        self.viewport._log_to_stream(stream)
        if hasattr(self, "blueprint_panel"):
            self.blueprint_panel._log_to_stream(stream)
        if hasattr(self, "selection_panel"):
            self.selection_panel._log_to_stream(stream)
        if hasattr(self, "time_panel"):
            self.time_panel._log_to_stream(stream)


BlueprintLike = Union[Blueprint, Viewport, Container, SpaceView]

"""
A type that can be converted to a blueprint.

These types all implement a `to_blueprint()` method that wraps them in the necessary
helper classes.
"""


def create_in_memory_blueprint(*, application_id: str, blueprint: BlueprintLike) -> MemoryRecording:
    """Internal rerun helper to convert a `BlueprintLike` into a stream that can be sent to the viewer."""

    # Convert the BlueprintLike to a full blueprint
    blueprint = blueprint.to_blueprint()

    blueprint_stream = RecordingStream(
        bindings.new_blueprint(
            application_id=application_id,
        )
    )

    blueprint_stream.set_time_sequence("blueprint", 0)  # type: ignore[attr-defined]

    blueprint._log_to_stream(blueprint_stream)

    return blueprint_stream.memory_recording()  # type: ignore[attr-defined, no-any-return]
