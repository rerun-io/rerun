# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/archetypes/viewport_blueprint.fbs".

# You can extend this class by creating a "ViewportBlueprintExt" class in "viewport_blueprint_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["ViewportBlueprint"]


@define(str=False, repr=False, init=False)
class ViewportBlueprint(Archetype):
    """**Archetype**: The top-level description of the Viewport."""

    def __init__(
        self: Any,
        *,
        root_container: datatypes.UuidLike | None = None,
        maximized: datatypes.UuidLike | None = None,
        auto_layout: blueprint_components.AutoLayoutLike | None = None,
        auto_space_views: blueprint_components.AutoSpaceViewsLike | None = None,
        past_viewer_recommendations: datatypes.UInt64ArrayLike | None = None,
    ):
        """
        Create a new instance of the ViewportBlueprint archetype.

        Parameters
        ----------
        root_container:
            The layout of the space-views
        maximized:
            Show one tab as maximized?
        auto_layout:
            Whether the viewport layout is determined automatically.

            If `true`, the container layout will be reset whenever a new space view is added or removed.
            This defaults to `false` and is automatically set to `false` when there is user determined layout.
        auto_space_views:
            Whether or not space views should be created automatically.

            If `true`, the viewer will only add space views that it hasn't considered previously (as identified by `past_viewer_recommendations`)
            and which aren't deemed redundant to existing space views.
            This defaults to `false` and is automatically set to `false` when the user adds space views manually in the viewer.
        past_viewer_recommendations:
            Hashes of all recommended space views the viewer has already added and that should not be added again.

            This is an internal field and should not be set usually.
            If you want the viewer from stopping to add space views, you should set `auto_space_views` to `false`.

            The viewer uses this to determine whether it should keep adding space views.

        """

        # You can define your own __init__ function as a member of ViewportBlueprintExt in viewport_blueprint_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                root_container=root_container,
                maximized=maximized,
                auto_layout=auto_layout,
                auto_space_views=auto_space_views,
                past_viewer_recommendations=past_viewer_recommendations,
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            root_container=None,  # type: ignore[arg-type]
            maximized=None,  # type: ignore[arg-type]
            auto_layout=None,  # type: ignore[arg-type]
            auto_space_views=None,  # type: ignore[arg-type]
            past_viewer_recommendations=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> ViewportBlueprint:
        """Produce an empty ViewportBlueprint, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    root_container: blueprint_components.RootContainerBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.RootContainerBatch._optional,  # type: ignore[misc]
    )
    # The layout of the space-views
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    maximized: blueprint_components.SpaceViewMaximizedBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.SpaceViewMaximizedBatch._optional,  # type: ignore[misc]
    )
    # Show one tab as maximized?
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    auto_layout: blueprint_components.AutoLayoutBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.AutoLayoutBatch._optional,  # type: ignore[misc]
    )
    # Whether the viewport layout is determined automatically.
    #
    # If `true`, the container layout will be reset whenever a new space view is added or removed.
    # This defaults to `false` and is automatically set to `false` when there is user determined layout.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    auto_space_views: blueprint_components.AutoSpaceViewsBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.AutoSpaceViewsBatch._optional,  # type: ignore[misc]
    )
    # Whether or not space views should be created automatically.
    #
    # If `true`, the viewer will only add space views that it hasn't considered previously (as identified by `past_viewer_recommendations`)
    # and which aren't deemed redundant to existing space views.
    # This defaults to `false` and is automatically set to `false` when the user adds space views manually in the viewer.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    past_viewer_recommendations: blueprint_components.ViewerRecommendationHashBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ViewerRecommendationHashBatch._optional,  # type: ignore[misc]
    )
    # Hashes of all recommended space views the viewer has already added and that should not be added again.
    #
    # This is an internal field and should not be set usually.
    # If you want the viewer from stopping to add space views, you should set `auto_space_views` to `false`.
    #
    # The viewer uses this to determine whether it should keep adding space views.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
