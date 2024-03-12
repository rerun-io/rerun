from __future__ import annotations

import itertools
import uuid
from typing import Iterable, Optional, Sequence, Union

import rerun_bindings as bindings

from ..datatypes import EntityPathLike, Utf8Like
from ..recording import MemoryRecording
from ..recording_stream import RecordingStream
from .archetypes import (
    ContainerBlueprint,
    SpaceViewBlueprint,
    SpaceViewContents,
    ViewportBlueprint,
)
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
        class_identifier: Utf8Like,
        origin: EntityPathLike,
        contents: SpaceViewContentsLike,
    ):
        """
        Construct a blueprint for a new space view.

        Parameters
        ----------
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
        self.origin = origin
        self.contents = contents

    def blueprint_path(self) -> str:
        """
        The blueprint path where this space view will be logged.

        Note that although this is an `EntityPath`, is is scoped to the blueprint tree and
        not a part of the regular data hierarchy.
        """
        return f"space_view/{self.id}"

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        # Handle the cases for SpaceViewContentsLike
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

    def __init__(self, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**"):
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

        """
        super().__init__("3D", origin, contents)


class Spatial2D(SpaceView):
    """A Spatial 2D space view."""

    def __init__(self, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**"):
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

        """
        super().__init__("2D", origin, contents)


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
            The layout shares of the columns in the container. This is only applicable to `Horizontal` or `Grid` containers.
        row_shares
            The layout shares of the rows in the container. This is only applicable to `Vertical` or `Grid` containers.
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

        Note that although this is an `EntityPath`, is is scoped to the blueprint tree and
        not a part of the regular data hierarchy.
        """
        return f"container/{self.id}"

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
            The layout shares of the columns in the container.

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
            The layout shares of the rows in the container.

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
            The layout shares of the columns in the container.
        row_shares
            The layout shares of the rows in the container.
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

    def __init__(self, root_container: Container):
        """
        Construct a new viewport.

        Parameters
        ----------
        root_container:
            The container that sits at the top of the viewport hierarchy. The only content visible
            in this viewport must be contained within this container.

        """
        self.root_container = root_container

    def blueprint_path(self) -> str:
        """
        The blueprint path where this space view will be logged.

        Note that although this is an `EntityPath`, is is scoped to the blueprint tree and
        not a part of the regular data hierarchy.
        """
        return "viewport"

    def _log_to_stream(self, stream: RecordingStream) -> None:
        """Internal method to convert to an archetype and log to the stream."""
        self.root_container._log_to_stream(stream)

        arch = ViewportBlueprint(
            space_views=list(self.root_container._iter_space_views()),
            root_container=self.root_container.id.bytes,
            auto_layout=False,
            auto_space_views=False,
        )

        stream.log(self.blueprint_path(), arch)  # type: ignore[attr-defined]


BlueprintLike = Union[Viewport, Container, SpaceView]


def create_in_memory_blueprint(*, application_id: str, blueprint: BlueprintLike) -> MemoryRecording:
    """Internal rerun helper to convert a `BlueprintLike` into a stream that can be sent to the viewer."""

    # Add trivial wrappers as necessary
    if isinstance(blueprint, SpaceView):
        blueprint = Viewport(Grid(blueprint))
    elif isinstance(blueprint, Container):
        blueprint = Viewport(blueprint)

    blueprint_stream = RecordingStream(
        bindings.new_blueprint(
            application_id=application_id,
        )
    )

    # TODO(jleibs): This should use a monotonic seq
    blueprint_stream.set_time_seconds("blueprint", 1)  # type: ignore[attr-defined]

    blueprint._log_to_stream(blueprint_stream)

    return blueprint_stream.memory_recording()  # type: ignore[attr-defined, no-any-return]
