# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/timeline.fbs".

# You can extend this class by creating a "TimelineExt" class in "timeline_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["Timeline", "TimelineBatch", "TimelineType"]


class Timeline(datatypes.Utf8, ComponentMixin):
    """**Component**: A timeline, identified by its name."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of TimelineExt in timeline_ext.py

    # Note: there are no fields here because Timeline delegates to datatypes.Utf8
    pass


class TimelineType(datatypes.Utf8Type):
    _TYPE_NAME: str = "rerun.blueprint.components.Timeline"


class TimelineBatch(datatypes.Utf8Batch, ComponentBatchMixin):
    _ARROW_TYPE = TimelineType()


# This is patched in late to avoid circular dependencies.
Timeline._BATCH_TYPE = TimelineBatch  # type: ignore[assignment]
