# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/view_blueprint.fbs".

# You can extend this class by creating a "ViewBlueprintExt" class in "view_blueprint_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import components, datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["ViewBlueprint"]


@define(str=False, repr=False, init=False)
class ViewBlueprint(Archetype):
    """**Archetype**: The description of a single view."""

    def __init__(
        self: Any,
        class_identifier: datatypes.Utf8Like,
        *,
        display_name: datatypes.Utf8Like | None = None,
        space_origin: datatypes.EntityPathLike | None = None,
        visible: datatypes.BoolLike | None = None,
    ):
        """
        Create a new instance of the ViewBlueprint archetype.

        Parameters
        ----------
        class_identifier:
            The class of the view.
        display_name:
            The name of the view.
        space_origin:
            The "anchor point" of this view.

            Defaults to the root path '/' if not specified.

            The transform at this path forms the reference point for all scene->world transforms in this view.
            I.e. the position of this entity path in space forms the origin of the coordinate system in this view.
            Furthermore, this is the primary indicator for heuristics on what entities we show in this view.
        visible:
            Whether this view is visible.

            Defaults to true if not specified.

        """

        # You can define your own __init__ function as a member of ViewBlueprintExt in view_blueprint_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                class_identifier=class_identifier, display_name=display_name, space_origin=space_origin, visible=visible
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            class_identifier=None,  # type: ignore[arg-type]
            display_name=None,  # type: ignore[arg-type]
            space_origin=None,  # type: ignore[arg-type]
            visible=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> ViewBlueprint:
        """Produce an empty ViewBlueprint, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        class_identifier: datatypes.Utf8Like | None = None,
        display_name: datatypes.Utf8Like | None = None,
        space_origin: datatypes.EntityPathLike | None = None,
        visible: datatypes.BoolLike | None = None,
    ) -> ViewBlueprint:
        """
        Update only some specific fields of a `ViewBlueprint`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        class_identifier:
            The class of the view.
        display_name:
            The name of the view.
        space_origin:
            The "anchor point" of this view.

            Defaults to the root path '/' if not specified.

            The transform at this path forms the reference point for all scene->world transforms in this view.
            I.e. the position of this entity path in space forms the origin of the coordinate system in this view.
            Furthermore, this is the primary indicator for heuristics on what entities we show in this view.
        visible:
            Whether this view is visible.

            Defaults to true if not specified.

        """

        kwargs = {
            "class_identifier": class_identifier,
            "display_name": display_name,
            "space_origin": space_origin,
            "visible": visible,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return ViewBlueprint(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> ViewBlueprint:
        """Clear all the fields of a `ViewBlueprint`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            class_identifier=[],
            display_name=[],
            space_origin=[],
            visible=[],
        )
        return inst

    class_identifier: blueprint_components.ViewClassBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ViewClassBatch._optional,  # type: ignore[misc]
    )
    # The class of the view.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    display_name: components.NameBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.NameBatch._optional,  # type: ignore[misc]
    )
    # The name of the view.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    space_origin: blueprint_components.ViewOriginBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ViewOriginBatch._optional,  # type: ignore[misc]
    )
    # The "anchor point" of this view.
    #
    # Defaults to the root path '/' if not specified.
    #
    # The transform at this path forms the reference point for all scene->world transforms in this view.
    # I.e. the position of this entity path in space forms the origin of the coordinate system in this view.
    # Furthermore, this is the primary indicator for heuristics on what entities we show in this view.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    visible: blueprint_components.VisibleBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.VisibleBatch._optional,  # type: ignore[misc]
    )
    # Whether this view is visible.
    #
    # Defaults to true if not specified.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
