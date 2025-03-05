# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/tensor_dimension_selection.fbs".

# You can extend this class by creating a "TensorWidthDimensionExt" class in "tensor_width_dimension_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["TensorWidthDimension", "TensorWidthDimensionBatch"]


class TensorWidthDimension(datatypes.TensorDimensionSelection, ComponentMixin):
    """**Component**: Specifies which dimension to use for width."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of TensorWidthDimensionExt in tensor_width_dimension_ext.py

    # Note: there are no fields here because TensorWidthDimension delegates to datatypes.TensorDimensionSelection


class TensorWidthDimensionBatch(datatypes.TensorDimensionSelectionBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.components.TensorWidthDimension")


# This is patched in late to avoid circular dependencies.
TensorWidthDimension._BATCH_TYPE = TensorWidthDimensionBatch  # type: ignore[assignment]
