from __future__ import annotations

from ..datatypes import EntityPathLike, Utf8Like
from .api import SpaceView, SpaceViewContentsLike


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
