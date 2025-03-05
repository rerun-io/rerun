# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/included_content.fbs".

# You can extend this class by creating a "IncludedContentExt" class in "included_content_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["IncludedContent", "IncludedContentBatch"]


class IncludedContent(datatypes.EntityPath, ComponentMixin):
    """**Component**: All the contents in the container."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of IncludedContentExt in included_content_ext.py

    # Note: there are no fields here because IncludedContent delegates to datatypes.EntityPath


class IncludedContentBatch(datatypes.EntityPathBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.IncludedContent")


# This is patched in late to avoid circular dependencies.
IncludedContent._BATCH_TYPE = IncludedContentBatch  # type: ignore[assignment]
