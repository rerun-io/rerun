# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/column_share.fbs".

# You can extend this class by creating a "ColumnShareExt" class in "column_share_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["ColumnShare", "ColumnShareBatch"]


class ColumnShare(datatypes.Float32, ComponentMixin):
    """**Component**: The layout share of a column in the container."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ColumnShareExt in column_share_ext.py

    # Note: there are no fields here because ColumnShare delegates to datatypes.Float32


class ColumnShareBatch(datatypes.Float32Batch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.ColumnShare")


# This is patched in late to avoid circular dependencies.
ColumnShare._BATCH_TYPE = ColumnShareBatch  # type: ignore[assignment]
