# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/container_blueprint.fbs".

# You can extend this class by creating a "ContainerBlueprintExt" class in "container_blueprint_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import components, datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["ContainerBlueprint"]


@define(str=False, repr=False, init=False)
class ContainerBlueprint(Archetype):
    """**Archetype**: The description of a container."""

    def __init__(
        self: Any,
        container_kind: blueprint_components.ContainerKindLike,
        *,
        display_name: datatypes.Utf8Like | None = None,
        contents: datatypes.EntityPathArrayLike | None = None,
        col_shares: datatypes.Float32ArrayLike | None = None,
        row_shares: datatypes.Float32ArrayLike | None = None,
        active_tab: datatypes.EntityPathLike | None = None,
        visible: datatypes.BoolLike | None = None,
        grid_columns: datatypes.UInt32Like | None = None,
    ):
        """
        Create a new instance of the ContainerBlueprint archetype.

        Parameters
        ----------
        container_kind:
            The class of the view.
        display_name:
            The name of the container.
        contents:
            `ContainerId`s or `SpaceViewId`s that are children of this container.
        col_shares:
            The layout shares of each column in the container.

            For [`components.ContainerKind.Horizontal`][rerun.blueprint.components.ContainerKind.Horizontal] containers, the length of this list should always match the number of contents.

            Ignored for [`components.ContainerKind.Vertical`][rerun.blueprint.components.ContainerKind.Vertical] containers.
        row_shares:
            The layout shares of each row of the container.

            For [`components.ContainerKind.Vertical`][rerun.blueprint.components.ContainerKind.Vertical] containers, the length of this list should always match the number of contents.

            Ignored for [`components.ContainerKind.Horizontal`][rerun.blueprint.components.ContainerKind.Horizontal] containers.
        active_tab:
            Which tab is active.

            Only applies to `Tabs` containers.
        visible:
            Whether this container is visible.

            Defaults to true if not specified.
        grid_columns:
            How many columns this grid should have.

            If unset, the grid layout will be auto.

            Ignored for [`components.ContainerKind.Horizontal`][rerun.blueprint.components.ContainerKind.Horizontal]/[`components.ContainerKind.Vertical`][rerun.blueprint.components.ContainerKind.Vertical] containers.

        """

        # You can define your own __init__ function as a member of ContainerBlueprintExt in container_blueprint_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                container_kind=container_kind,
                display_name=display_name,
                contents=contents,
                col_shares=col_shares,
                row_shares=row_shares,
                active_tab=active_tab,
                visible=visible,
                grid_columns=grid_columns,
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            container_kind=None,  # type: ignore[arg-type]
            display_name=None,  # type: ignore[arg-type]
            contents=None,  # type: ignore[arg-type]
            col_shares=None,  # type: ignore[arg-type]
            row_shares=None,  # type: ignore[arg-type]
            active_tab=None,  # type: ignore[arg-type]
            visible=None,  # type: ignore[arg-type]
            grid_columns=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> ContainerBlueprint:
        """Produce an empty ContainerBlueprint, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    container_kind: blueprint_components.ContainerKindBatch = field(
        metadata={"component": "required"},
        converter=blueprint_components.ContainerKindBatch._required,  # type: ignore[misc]
    )
    # The class of the view.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    display_name: components.NameBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.NameBatch._optional,  # type: ignore[misc]
    )
    # The name of the container.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    contents: blueprint_components.IncludedContentBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.IncludedContentBatch._optional,  # type: ignore[misc]
    )
    # `ContainerId`s or `SpaceViewId`s that are children of this container.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    col_shares: blueprint_components.ColumnShareBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ColumnShareBatch._optional,  # type: ignore[misc]
    )
    # The layout shares of each column in the container.
    #
    # For [`components.ContainerKind.Horizontal`][rerun.blueprint.components.ContainerKind.Horizontal] containers, the length of this list should always match the number of contents.
    #
    # Ignored for [`components.ContainerKind.Vertical`][rerun.blueprint.components.ContainerKind.Vertical] containers.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    row_shares: blueprint_components.RowShareBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.RowShareBatch._optional,  # type: ignore[misc]
    )
    # The layout shares of each row of the container.
    #
    # For [`components.ContainerKind.Vertical`][rerun.blueprint.components.ContainerKind.Vertical] containers, the length of this list should always match the number of contents.
    #
    # Ignored for [`components.ContainerKind.Horizontal`][rerun.blueprint.components.ContainerKind.Horizontal] containers.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    active_tab: blueprint_components.ActiveTabBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.ActiveTabBatch._optional,  # type: ignore[misc]
    )
    # Which tab is active.
    #
    # Only applies to `Tabs` containers.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    visible: blueprint_components.VisibleBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.VisibleBatch._optional,  # type: ignore[misc]
    )
    # Whether this container is visible.
    #
    # Defaults to true if not specified.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    grid_columns: blueprint_components.GridColumnsBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.GridColumnsBatch._optional,  # type: ignore[misc]
    )
    # How many columns this grid should have.
    #
    # If unset, the grid layout will be auto.
    #
    # Ignored for [`components.ContainerKind.Horizontal`][rerun.blueprint.components.ContainerKind.Horizontal]/[`components.ContainerKind.Vertical`][rerun.blueprint.components.ContainerKind.Vertical] containers.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
