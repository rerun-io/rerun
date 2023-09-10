# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)

__all__ = ["Float32", "Float32Array", "Float32ArrayLike", "Float32Like", "Float32Type"]


@define
class Float32:
    value: float = field(converter=float)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)


Float32Like = Float32
Float32ArrayLike = Union[
    Float32,
    Sequence[Float32Like],
]


# --- Arrow support ---


class Float32Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), "rerun.datatypes.Float32")


class Float32Array(BaseExtensionArray[Float32ArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Float32"
    _EXTENSION_TYPE = Float32Type

    @staticmethod
    def _native_to_pa_array(data: Float32ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "float32_native_to_pa_array" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/float32.py


Float32Type._ARRAY_TYPE = Float32Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Float32Type())
