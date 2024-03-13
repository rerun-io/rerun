from __future__ import annotations

from ..datatypes import EntityPathLike, Utf8Like
from .api import SpaceView, SpaceViewContentsLike


class BarChartView(SpaceView):
    """A bar chart view."""

    def __init__(
        self, *, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**", name: Utf8Like | None = None
    ):
        """
        Construct a blueprint for a new bar chart view.

        Parameters
        ----------
        origin
            The `EntityPath` to use as the origin of this view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.
            See: [rerun.blueprint.components.QueryExpression][].
        name
            The name of the view.

        """
        super().__init__(class_identifier="Bar Chart", origin=origin, contents=contents, name=name)


class Spatial2DView(SpaceView):
    """A Spatial 2D view."""

    def __init__(
        self, *, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**", name: Utf8Like | None = None
    ):
        """
        Construct a blueprint for a new spatial 2D view.

        Parameters
        ----------
        origin
            The `EntityPath` to use as the origin of this view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.
            See: [rerun.blueprint.components.QueryExpression][].
        name
            The name of the view.

        """
        super().__init__(class_identifier="2D", origin=origin, contents=contents, name=name)


class Spatial3DView(SpaceView):
    """A Spatial 3D view."""

    def __init__(
        self, *, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**", name: Utf8Like | None = None
    ):
        """
        Construct a blueprint for a new spatial 3D view.

        Parameters
        ----------
        origin
            The `EntityPath` to use as the origin of this view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.
            See: [rerun.blueprint.components.QueryExpression][].
        name
            The name of the view.

        """
        super().__init__(class_identifier="3D", origin=origin, contents=contents, name=name)


class TensorView(SpaceView):
    """A tensor view."""

    def __init__(
        self, *, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**", name: Utf8Like | None = None
    ):
        """
        Construct a blueprint for a new tensor view.

        Parameters
        ----------
        origin
            The `EntityPath` to use as the origin of this view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.
            See: [rerun.blueprint.components.QueryExpression][].
        name
            The name of the view.

        """
        super().__init__(class_identifier="Tensor", origin=origin, contents=contents, name=name)


class TextDocumentView(SpaceView):
    """A text document view."""

    def __init__(
        self, *, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**", name: Utf8Like | None = None
    ):
        """
        Construct a blueprint for a new text document view.

        Parameters
        ----------
        origin
            The `EntityPath` to use as the origin of this view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.
            See: [rerun.blueprint.components.QueryExpression][].
        name
            The name of the view.

        """
        super().__init__(class_identifier="Text Document", origin=origin, contents=contents, name=name)


class TextLogView(SpaceView):
    """A text log view."""

    def __init__(
        self, *, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**", name: Utf8Like | None = None
    ):
        """
        Construct a blueprint for a new text log view.

        Parameters
        ----------
        origin
            The `EntityPath` to use as the origin of this view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.
            See: [rerun.blueprint.components.QueryExpression][].
        name
            The name of the view.

        """
        super().__init__(class_identifier="TextLog", origin=origin, contents=contents, name=name)


class TimeSeriesView(SpaceView):
    """A time series view."""

    def __init__(
        self, *, origin: EntityPathLike = "/", contents: SpaceViewContentsLike = "/**", name: Utf8Like | None = None
    ):
        """
        Construct a blueprint for a new time series view.

        Parameters
        ----------
        origin
            The `EntityPath` to use as the origin of this view. All other entities will be transformed
            to be displayed relative to this origin.
        contents
            The contents of the view. Most commonly specified as a query expression. The individual
            sub-expressions must either be newline separate, or provided as a list of strings.
            See: [rerun.blueprint.components.QueryExpression][].
        name
            The name of the view.

        """
        super().__init__(class_identifier="Time Series", origin=origin, contents=contents, name=name)
