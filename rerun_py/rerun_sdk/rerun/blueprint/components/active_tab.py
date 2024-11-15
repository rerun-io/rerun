# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/active_tab.fbs".

# You can extend this class by creating a "ActiveTabExt" class in "active_tab_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["ActiveTab", "ActiveTabBatch"]


class ActiveTab(datatypes.EntityPath, ComponentMixin):
    """**Component**: The active tab in a tabbed container."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ActiveTabExt in active_tab_ext.py

    # Note: there are no fields here because ActiveTab delegates to datatypes.EntityPath
    pass


class ActiveTabBatch(datatypes.EntityPathBatch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.blueprint.components.ActiveTab"


# This is patched in late to avoid circular dependencies.
ActiveTab._BATCH_TYPE = ActiveTabBatch  # type: ignore[assignment]
