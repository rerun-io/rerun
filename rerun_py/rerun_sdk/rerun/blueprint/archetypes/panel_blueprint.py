# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/panel_blueprint.fbs".

# You can extend this class by creating a "PanelBlueprintExt" class in "panel_blueprint_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["PanelBlueprint"]


@define(str=False, repr=False, init=False)
class PanelBlueprint(Archetype):
    """**Archetype**: Shared state for the 3 collapsible panels."""

    def __init__(self: Any, *, state: blueprint_components.PanelStateLike | None = None):
        """
        Create a new instance of the PanelBlueprint archetype.

        Parameters
        ----------
        state:
            Current state of the panels.

        """

        # You can define your own __init__ function as a member of PanelBlueprintExt in panel_blueprint_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(state=state)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            state=None,
        )

    @classmethod
    def _clear(cls) -> PanelBlueprint:
        """Produce an empty PanelBlueprint, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        state: blueprint_components.PanelStateLike | None = None,
    ) -> PanelBlueprint:
        """
        Update only some specific fields of a `PanelBlueprint`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        state:
            Current state of the panels.

        """

        kwargs = {
            "state": state,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return PanelBlueprint(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> PanelBlueprint:
        """Clear all the fields of a `PanelBlueprint`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            state=[],
        )
        return inst

    state: blueprint_components.PanelStateBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=blueprint_components.PanelStateBatch._converter,  # type: ignore[misc]
    )
    # Current state of the panels.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
