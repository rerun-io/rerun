from __future__ import annotations

import itertools
import uuid
from typing import Iterable, Optional, Sequence, Union

import rerun_bindings as bindings

from ..datatypes import EntityPathLike, Utf8Like
from ..recording import MemoryRecording
from ..recording_stream import RecordingStream
from .archetypes import ContainerBlueprint, PanelBlueprint, SpaceViewBlueprint, SpaceViewContents, ViewportBlueprint
from .components import ColumnShareArrayLike, RowShareArrayLike
from .components.container_kind import ContainerKind, ContainerKindLike

SpaceViewContentsLike = Union[str, Sequence[str], Utf8Like, SpaceViewContents]


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
        return Viewport(Grid(self))

    def to_app_blueprint(self) -> App:
        """Convert this space view to a full app blueprint."""
        return App(self.to_viewport())

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        # Handle the cases for SpaceViewContentsLike
        # TODO(#5483): Move this into a QueryExpressionExt class.
        # This is a little bit tricky since QueryExpression is a delegating component for Utf8,
        # and delegating components make extending things in this way a bit more complicated.
        if isinstance(self.contents, str):
            # str
            contents = SpaceViewContents(query=self.contents)
        elif isinstance(self.contents, Sequence) and len(self.contents) > 0 and isinstance(self.contents[0], str):
            # list[str]
            contents = SpaceViewContents(query="\n".join(self.contents))
        elif isinstance(self.contents, SpaceViewContents):
            # SpaceViewContents
            contents = self.contents
        else:
            # Anything else we let SpaceViewContents handle
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


class Spatial3D(SpaceView):
    """A Spatial 3D space view."""

    def __init__(
        self, *, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**", name: Utf8Like | None = None
    ):
        """
        Construct a blueprint for a new 3D space view.

        Parameters
        ----------
        origin
            The `EntityPath` to use as the origin of this space view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the space view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.
            See: [rerun.blueprint.components.QueryExpression][].
        name
            The name of the space view.

        """
        super().__init__(class_identifier="3D", origin=origin, contents=contents, name=name)


class Spatial2D(SpaceView):
    """A Spatial 2D space view."""

    def __init__(
        self, *, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**", name: Utf8Like | None = None
    ):
        """
        Construct a blueprint for a new 2D space view.

        Parameters
        ----------
        origin
            The `EntityPath` to use as the origin of this space view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the space view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.
            See: [rerun.blueprint.components.QueryExpression][].
        name
            The name of the space view.

        """
        super().__init__(class_identifier="2D", origin=origin, contents=contents, name=name)


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
            row should take up. The ros with index `i` will take up the fraction `shares[i] / total_shares`.
            This is only applicable to `Vertical` or `Grid` containers.
        grid_columns
            The number of columns in the grid. This is only applicable to `Grid` containers.

        """
        self.id = uuid.uuid4()
        self.kind = kind
        self.contents = contents
        self.column_shares = column_shares
        self.row_shares = row_shares
        self.grid_columns = grid_columns

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

    def to_app_blueprint(self) -> App:
        """Convert this container to a full app blueprint."""
        return App(self.to_viewport())

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        for sub in self.contents:
            sub._log_to_stream(stream)

        arch = ContainerBlueprint(
            container_kind=self.kind,
            contents=[sub.blueprint_path() for sub in self.contents],
            col_shares=self.column_shares,
            row_shares=self.row_shares,
            visible=True,
            grid_columns=self.grid_columns,
        )

        stream.log(self.blueprint_path(), arch)  # type: ignore[attr-defined]

    def _iter_space_views(self) -> Iterable[bytes]:
        """Internal method to iterate over all of the space views in the blueprint."""
        # TODO(jleibs): This goes away when we get rid of `space_views` from the viewport and just use
        # the entity-path lookup instead.
        return itertools.chain.from_iterable(sub._iter_space_views() for sub in self.contents)


class Horizontal(Container):
    """A horizontal container."""

    def __init__(self, *contents: Container | SpaceView, column_shares: Optional[ColumnShareArrayLike] = None):
        """
        Construct a new horizontal container.

        Parameters
        ----------
        *contents:
            All positional arguments are the contents of the container, which may be either other containers or space views.
        column_shares
            The layout shares of the columns in the container. The share is used to determine what fraction of the total width each
            column should take up. The column with index `i` will take up the fraction `shares[i] / total_shares`.

        """
        super().__init__(*contents, kind=ContainerKind.Horizontal, column_shares=column_shares)


class Vertical(Container):
    """A vertical container."""

    def __init__(self, *contents: Container | SpaceView, row_shares: Optional[RowShareArrayLike] = None):
        """
        Construct a new vertical container.

        Parameters
        ----------
        *contents:
            All positional arguments are the contents of the container, which may be either other containers or space views.
        row_shares
            The layout shares of the rows in the container. The share is used to determine what fraction of the total height each
            row should take up. The ros with index `i` will take up the fraction `shares[i] / total_shares`.

        """
        super().__init__(*contents, kind=ContainerKind.Vertical, row_shares=row_shares)


class Grid(Container):
    """A grid container."""

    def __init__(
        self,
        *contents: Container | SpaceView,
        column_shares: Optional[ColumnShareArrayLike] = None,
        row_shares: Optional[RowShareArrayLike] = None,
        grid_columns: Optional[int] = None,
    ):
        """
        Construct a new grid container.

        Parameters
        ----------
        *contents:
            All positional arguments are the contents of the container, which may be either other containers or space views.
        column_shares
            The layout shares of the columns in the container. The share is used to determine what fraction of the total width each
            column should take up. The column with index `i` will take up the fraction `shares[i] / total_shares`.
        row_shares
            The layout shares of the rows in the container. The share is used to determine what fraction of the total height each
            row should take up. The ros with index `i` will take up the fraction `shares[i] / total_shares`.
        grid_columns
            The number of columns in the grid.

        """
        super().__init__(
            *contents,
            kind=ContainerKind.Grid,
            column_shares=column_shares,
            row_shares=row_shares,
            grid_columns=grid_columns,
        )


class Tabs(Container):
    """A tab container."""

    def __init__(self, *contents: Container | SpaceView):
        """
        Construct a new tab container.

        Parameters
        ----------
        *contents:
            All positional arguments are the contents of the container, which may be either other containers or space views.

        """
        super().__init__(*contents, kind=ContainerKind.Tabs)


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

    def to_app_blueprint(self) -> App:
        """Convert this viewport to a full app blueprint."""
        return App(self)

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
    The state of a panel in the app.

    This is used internally by the app to control the state of the 3 main panels.
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


ViewportLike = Union[Viewport, Container, SpaceView]
"""
A type that can be converted to a viewport.

These types all implement a `to_viewport()` method that wraps them in the necessary
helper classes.
"""


class App:
    """
    The top-level description of the viewer application.

    The app allows you to specify a viewport and control the state of the 3 main panels.
    """

    def __init__(
        self,
        viewport: ViewportLike,
        *,
        blueprint_panel_expanded: bool | None = None,
        selection_panel_expanded: bool | None = None,
        time_panel_expanded: bool | None = None,
    ):
        """
        Construct a new app.

        Parameters
        ----------
        viewport:
            The viewport that will be displayed in the app.
        blueprint_panel_expanded:
            Whether the blueprint panel is expanded or not. If unset the panel will choose its state based on the window size.
        selection_panel_expanded:
            Whether the selection panel is expanded or not. If unset the panel will choose its state based on the window size.
        time_panel_expanded:
            Whether the time panel is expanded or not. If unset the panel will choose its state based on the window size.

        """

        self.viewport = viewport.to_viewport()

        self.blueprint_panel = Panel(
            blueprint_path="blueprint_panel",
            expanded=blueprint_panel_expanded,
        )
        self.selection_panel = Panel(
            blueprint_path="selection_panel",
            expanded=selection_panel_expanded,
        )
        self.time_panel = Panel(
            blueprint_path="time_panel",
            expanded=time_panel_expanded,
        )

    def to_app_blueprint(self) -> App:
        """Conform with the `BlueprintLike` interface."""
        return self

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        self.viewport._log_to_stream(stream)
        self.blueprint_panel._log_to_stream(stream)
        self.selection_panel._log_to_stream(stream)
        self.time_panel._log_to_stream(stream)


BlueprintLike = Union[App, Viewport, Container, SpaceView]
"""
A type that can be converted to a blueprint.

These types all implement a `to_app_blueprint()` method that wraps them in the necessary
helper classes.
"""


def create_in_memory_blueprint(*, application_id: str, blueprint: BlueprintLike) -> MemoryRecording:
    """Internal rerun helper to convert a `BlueprintLike` into a stream that can be sent to the viewer."""

    # Convert the BlueprintLike to a full app blueprint
    blueprint = blueprint.to_app_blueprint()

    blueprint_stream = RecordingStream(
        bindings.new_blueprint(
            application_id=application_id,
        )
    )

    # TODO(jleibs): This should use a monotonic seq
    blueprint_stream.set_time_seconds("blueprint", 1)  # type: ignore[attr-defined]

    blueprint._log_to_stream(blueprint_stream)

    return blueprint_stream.memory_recording()  # type: ignore[attr-defined, no-any-return]
