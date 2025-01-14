# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/viewport_blueprint.fbs".

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
    """**Archetype**: The top-level description of the viewport."""

    def __init__(
        self: Any,
        *,
        root_container: datatypes.UuidLike | None = None,
        maximized: datatypes.UuidLike | None = None,
        auto_layout: datatypes.BoolLike | None = None,
        auto_views: datatypes.BoolLike | None = None,
        past_viewer_recommendations: datatypes.UInt64ArrayLike | None = None,
    ):
        """
        Create a new instance of the ViewportBlueprint archetype.

        Parameters
        ----------
        root_container:
            The layout of the views
        maximized:
            Show one tab as maximized?
        auto_layout:
            Whether the viewport layout is determined automatically.

            If `true`, the container layout will be reset whenever a new view is added or removed.
            This defaults to `false` and is automatically set to `false` when there is user determined layout.
        auto_views:
            Whether or not views should be created automatically.

            If `true`, the viewer will only add views that it hasn't considered previously (as identified by `past_viewer_recommendations`)
            and which aren't deemed redundant to existing views.
            This defaults to `false` and is automatically set to `false` when the user adds views manually in the viewer.
        past_viewer_recommendations:
            Hashes of all recommended views the viewer has already added and that should not be added again.

            This is an internal field and should not be set usually.
            If you want the viewer from stopping to add views, you should set `auto_views` to `false`.

            The viewer uses this to determine whether it should keep adding views.

        """

        # You can define your own __init__ function as a member of ViewportBlueprintExt in viewport_blueprint_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                root_container=root_container,
                maximized=maximized,
                auto_layout=auto_layout,
                auto_views=auto_views,
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
            auto_views=None,  # type: ignore[arg-type]
            past_viewer_recommendations=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> ViewportBlueprint:
        """Produce an empty ViewportBlueprint, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        root_container: datatypes.UuidLike | None = None,
        maximized: datatypes.UuidLike | None = None,
        auto_layout: datatypes.BoolLike | None = None,
        auto_views: datatypes.BoolLike | None = None,
        past_viewer_recommendations: datatypes.UInt64ArrayLike | None = None,
    ) -> ViewportBlueprint:
        """
        Update only some specific fields of a `ViewportBlueprint`.

        Parameters
        ----------
        clear:
             If true, all unspecified fields will be explicitly cleared.
        root_container:
            The layout of the views
        maximized:
            Show one tab as maximized?
        auto_layout:
            Whether the viewport layout is determined automatically.

            If `true`, the container layout will be reset whenever a new view is added or removed.
            This defaults to `false` and is automatically set to `false` when there is user determined layout.
        auto_views:
            Whether or not views should be created automatically.

            If `true`, the viewer will only add views that it hasn't considered previously (as identified by `past_viewer_recommendations`)
            and which aren't deemed redundant to existing views.
            This defaults to `false` and is automatically set to `false` when the user adds views manually in the viewer.
        past_viewer_recommendations:
            Hashes of all recommended views the viewer has already added and that should not be added again.

            This is an internal field and should not be set usually.
            If you want the viewer from stopping to add views, you should set `auto_views` to `false`.

            The viewer uses this to determine whether it should keep adding views.

        """

        kwargs = {
            "root_container": root_container,
            "maximized": maximized,
            "auto_layout": auto_layout,
            "auto_views": auto_views,
            "past_viewer_recommendations": past_viewer_recommendations,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return ViewportBlueprint(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> ViewportBlueprint:
        """Clear all the fields of a `ViewportBlueprint`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            root_container=[],  # type: ignore[arg-type]
            maximized=[],  # type: ignore[arg-type]
            auto_layout=[],  # type: ignore[arg-type]
            auto_views=[],  # type: ignore[arg-type]
            past_viewer_recommendations=[],  # type: ignore[arg-type]
        )
        return inst

    root_container: blueprint_components.RootContainerBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.RootContainerBatch._optional,  # type: ignore[misc]
    )
    # The layout of the views
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    maximized: blueprint_components.ViewMaximizedBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ViewMaximizedBatch._optional,  # type: ignore[misc]
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
    # If `true`, the container layout will be reset whenever a new view is added or removed.
    # This defaults to `false` and is automatically set to `false` when there is user determined layout.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    auto_views: blueprint_components.AutoViewsBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.AutoViewsBatch._optional,  # type: ignore[misc]
    )
    # Whether or not views should be created automatically.
    #
    # If `true`, the viewer will only add views that it hasn't considered previously (as identified by `past_viewer_recommendations`)
    # and which aren't deemed redundant to existing views.
    # This defaults to `false` and is automatically set to `false` when the user adds views manually in the viewer.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    past_viewer_recommendations: blueprint_components.ViewerRecommendationHashBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ViewerRecommendationHashBatch._optional,  # type: ignore[misc]
    )
    # Hashes of all recommended views the viewer has already added and that should not be added again.
    #
    # This is an internal field and should not be set usually.
    # If you want the viewer from stopping to add views, you should set `auto_views` to `false`.
    #
    # The viewer uses this to determine whether it should keep adding views.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
