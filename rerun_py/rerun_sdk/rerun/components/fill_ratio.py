# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/fill_ratio.fbs".

# You can extend this class by creating a "FillRatioExt" class in "fill_ratio_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["FillRatio", "FillRatioBatch", "FillRatioType"]


class FillRatio(datatypes.Float32, ComponentMixin):
    """
    **Component**: How much a primitive fills out the available space.

    Used for instance to scale the points of the point cloud created from `DepthImage` projection.
    Valid range is from 0 to max float although typically values above 1.0 are not useful.

    Defaults to 1.0.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of FillRatioExt in fill_ratio_ext.py

    # Note: there are no fields here because FillRatio delegates to datatypes.Float32
    pass


class FillRatioType(datatypes.Float32Type):
    _TYPE_NAME: str = "rerun.components.FillRatio"


class FillRatioBatch(datatypes.Float32Batch, ComponentBatchMixin):
    _ARROW_TYPE = FillRatioType()


# This is patched in late to avoid circular dependencies.
FillRatio._BATCH_TYPE = FillRatioBatch  # type: ignore[assignment]
