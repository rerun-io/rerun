# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/tensor_dimension_selection.fbs".

# You can extend this class by creating a "TensorDimensionSelectionExt" class in "tensor_dimension_selection_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, Union

import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseBatch,
)
from .tensor_dimension_selection_ext import TensorDimensionSelectionExt

__all__ = [
    "TensorDimensionSelection",
    "TensorDimensionSelectionArrayLike",
    "TensorDimensionSelectionBatch",
    "TensorDimensionSelectionLike",
]


@define(init=False)
class TensorDimensionSelection(TensorDimensionSelectionExt):
    """**Datatype**: Selection of a single tensor dimension."""

    # __init__ can be found in tensor_dimension_selection_ext.py

    dimension: int = field(
        converter=int,
    )
    # The dimension number to select.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    invert: bool = field(
        converter=bool,
    )
    # Invert the direction of the dimension.
    #
    # (Docstring intentionally commented out to hide this field from the docs)


if TYPE_CHECKING:
    TensorDimensionSelectionLike = Union[
        TensorDimensionSelection,
        int,
    ]
else:
    TensorDimensionSelectionLike = Any

TensorDimensionSelectionArrayLike = Union[
    TensorDimensionSelection,
    Sequence[TensorDimensionSelectionLike],
    npt.ArrayLike,
]


class TensorDimensionSelectionBatch(BaseBatch[TensorDimensionSelectionArrayLike]):
    _ARROW_DATATYPE = pa.struct([
        pa.field("dimension", pa.uint32(), nullable=False, metadata={}),
        pa.field("invert", pa.bool_(), nullable=False, metadata={}),
    ])

    @staticmethod
    def _native_to_pa_array(data: TensorDimensionSelectionArrayLike, data_type: pa.DataType) -> pa.Array:
        return TensorDimensionSelectionExt.native_to_pa_array_override(data, data_type)
