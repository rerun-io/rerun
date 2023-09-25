# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/scalars.fbs".

# You can extend this class by creating a "Float32Ext" class in "float32_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType

__all__ = ["Float32", "Float32ArrayLike", "Float32Batch", "Float32Like", "Float32Type"]


@define
class Float32:
    # You can define your own __init__ function as a member of Float32Ext in float32_ext.py

    value: float = field(converter=float)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of Float32Ext in float32_ext.py
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)


Float32Like = Float32
Float32ArrayLike = Union[
    Float32,
    Sequence[Float32Like],
]


class Float32Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Float32"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), self._TYPE_NAME)


class Float32Batch(BaseBatch[Float32ArrayLike]):
    _ARROW_TYPE = Float32Type()

    @staticmethod
    def _native_to_pa_array(data: Float32ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in float32_ext.py


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Float32Type())
