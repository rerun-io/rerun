# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy_deps.fbs".

# You can extend this class by creating a "PrimitiveComponentExt" class in "primitive_component_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import (
    BaseBatch,
)

__all__ = ["PrimitiveComponent", "PrimitiveComponentArrayLike", "PrimitiveComponentBatch", "PrimitiveComponentLike"]


@define(init=False)
class PrimitiveComponent:
    def __init__(self: Any, value: PrimitiveComponentLike) -> None:
        """Create a new instance of the PrimitiveComponent datatype."""

        # You can define your own __init__ function as a member of PrimitiveComponentExt in primitive_component_ext.py
        self.__attrs_init__(value=value)

    value: int = field(converter=int)

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of PrimitiveComponentExt in primitive_component_ext.py
        return np.asarray(self.value, dtype=dtype, copy=copy)

    def __int__(self) -> int:
        return int(self.value)

    def __hash__(self) -> int:
        return hash(self.value)


PrimitiveComponentLike = PrimitiveComponent
PrimitiveComponentArrayLike = Union[
    PrimitiveComponent,
    Sequence[PrimitiveComponentLike],
]


class PrimitiveComponentBatch(BaseBatch[PrimitiveComponentArrayLike]):
    _ARROW_DATATYPE = pa.uint32()

    @staticmethod
    def _native_to_pa_array(data: PrimitiveComponentArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, PrimitiveComponent):
            data = [data]

        return pa.StructArray.from_arrays(
            [
                pa.array(np.asarray([x.value for x in data], dtype=np.uint32)),
            ],
            fields=list(data_type),
        )
