# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

# You can extend this class by creating a "FlattenedScalarExt" class in "flattened_scalar_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import BaseBatch, BaseExtensionType

__all__ = [
    "FlattenedScalar",
    "FlattenedScalarArrayLike",
    "FlattenedScalarBatch",
    "FlattenedScalarLike",
    "FlattenedScalarType",
]


@define
class FlattenedScalar:
    # You can define your own __init__ function as a member of FlattenedScalarExt in flattened_scalar_ext.py

    value: float = field(converter=float)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of FlattenedScalarExt in flattened_scalar_ext.py
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)


FlattenedScalarLike = FlattenedScalar
FlattenedScalarArrayLike = Union[
    FlattenedScalar,
    Sequence[FlattenedScalarLike],
]


class FlattenedScalarType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.datatypes.FlattenedScalar"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.struct([pa.field("value", pa.float32(), nullable=False, metadata={})]), self._TYPE_NAME
        )


class FlattenedScalarBatch(BaseBatch[FlattenedScalarArrayLike]):
    _ARROW_TYPE = FlattenedScalarType()

    @staticmethod
    def _native_to_pa_array(data: FlattenedScalarArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in flattened_scalar_ext.py


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(FlattenedScalarType())
