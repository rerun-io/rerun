# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/archetypes/panel_blueprint.fbs".

# You can extend this class by creating a "PanelBlueprintExt" class in "panel_blueprint_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ..._baseclasses import Archetype
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["PanelBlueprint"]


@define(str=False, repr=False, init=False)
class PanelBlueprint(Archetype):
    """**Archetype**: Shared state for the 3 collapsible panels."""

    def __init__(self: Any, *, state: blueprint_components.PanelStateLike | None = None):
        """Create a new instance of the PanelBlueprint archetype."""

        # You can define your own __init__ function as a member of PanelBlueprintExt in panel_blueprint_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(state=state)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            state=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> PanelBlueprint:
        """Produce an empty PanelBlueprint, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    state: blueprint_components.PanelStateBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.PanelStateBatch._optional,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
