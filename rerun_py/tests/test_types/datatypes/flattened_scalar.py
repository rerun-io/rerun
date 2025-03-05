# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

# You can extend this class by creating a "FlattenedScalarExt" class in "flattened_scalar_ext.py".

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

__all__ = ["FlattenedScalar", "FlattenedScalarArrayLike", "FlattenedScalarBatch", "FlattenedScalarLike"]


@define(init=False)
class FlattenedScalar:
    def __init__(self: Any, value: FlattenedScalarLike) -> None:
        """Create a new instance of the FlattenedScalar datatype."""

        # You can define your own __init__ function as a member of FlattenedScalarExt in flattened_scalar_ext.py
        self.__attrs_init__(value=value)

    value: float = field(
        converter=float,
    )

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of FlattenedScalarExt in flattened_scalar_ext.py
        return np.asarray(self.value, dtype=dtype, copy=copy)

    def __float__(self) -> float:
        return float(self.value)

    def __hash__(self) -> int:
        return hash(self.value)


FlattenedScalarLike = FlattenedScalar
FlattenedScalarArrayLike = Union[
    FlattenedScalar,
    Sequence[FlattenedScalarLike],
]


class FlattenedScalarBatch(BaseBatch[FlattenedScalarArrayLike]):
    _ARROW_DATATYPE = pa.struct([pa.field("value", pa.float32(), nullable=False, metadata={})])

    @staticmethod
    def _native_to_pa_array(data: FlattenedScalarArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, FlattenedScalar):
            data = [data]

        return pa.StructArray.from_arrays(
            [
                pa.array(np.asarray([x.value for x in data], dtype=np.float32)),
            ],
            fields=list(data_type),
        )
