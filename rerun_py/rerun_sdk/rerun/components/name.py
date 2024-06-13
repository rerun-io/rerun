# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/name.fbs".

# You can extend this class by creating a "NameExt" class in "name_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["Name", "NameBatch", "NameType"]


class Name(datatypes.Utf8, ComponentMixin):
    """**Component**: A display name, typically for an entity or a item like a plot series."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of NameExt in name_ext.py

    # Note: there are no fields here because Name delegates to datatypes.Utf8
    pass


class NameType(datatypes.Utf8Type):
    _TYPE_NAME: str = "rerun.components.Name"


class NameBatch(datatypes.Utf8Batch, ComponentBatchMixin):
    _ARROW_TYPE = NameType()


# This is patched in late to avoid circular dependencies.
Name._BATCH_TYPE = NameBatch  # type: ignore[assignment]
