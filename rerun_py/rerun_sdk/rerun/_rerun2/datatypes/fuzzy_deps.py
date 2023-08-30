# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from typing import (Any, Dict, Iterable, Optional, Sequence, Set, Tuple, Union,
    TYPE_CHECKING, SupportsFloat, Literal)

from attrs import define, field
import numpy as np
import numpy.typing as npt
import pyarrow as pa

from .._baseclasses import (
    Archetype,
    BaseExtensionType,
    BaseExtensionArray,
    BaseDelegatingExtensionType,
    BaseDelegatingExtensionArray
)
from .._converters import (
    int_or_none,
    float_or_none,
    bool_or_none,
    str_or_none,
    to_np_uint8,
    to_np_uint16,
    to_np_uint32,
    to_np_uint64,
    to_np_int8,
    to_np_int16,
    to_np_int32,
    to_np_int64,
    to_np_bool,
    to_np_float16,
    to_np_float32,
    to_np_float64
)
__all__ = ["PrimitiveComponent", "PrimitiveComponentArray", "PrimitiveComponentArrayLike", "PrimitiveComponentLike", "PrimitiveComponentType", "StringComponent", "StringComponentArray", "StringComponentArrayLike", "StringComponentLike", "StringComponentType"]


@define
class PrimitiveComponent:
    value: int = field(converter=int)
    def __array__(self, dtype: npt.DTypeLike=None) -> npt.NDArray[Any]:
        return np.asarray(self.value, dtype=dtype)

    def __int__(self) -> int:
        return int(self.value)

PrimitiveComponentLike = PrimitiveComponent
PrimitiveComponentArrayLike = Union[
    PrimitiveComponent,
    Sequence[PrimitiveComponentLike],
    
]


# --- Arrow support ---

class PrimitiveComponentType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.uint32(), "rerun.testing.datatypes.PrimitiveComponent"
        )

class PrimitiveComponentArray(BaseExtensionArray[PrimitiveComponentArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.PrimitiveComponent"
    _EXTENSION_TYPE = PrimitiveComponentType

    @staticmethod
    def _native_to_pa_array(data: PrimitiveComponentArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError

PrimitiveComponentType._ARRAY_TYPE = PrimitiveComponentArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(PrimitiveComponentType())



@define
class StringComponent:
    value: str = field(converter=str)

    def __str__(self) -> str:
        return str(self.value)

StringComponentLike = StringComponent
StringComponentArrayLike = Union[
    StringComponent,
    Sequence[StringComponentLike],
    
]


# --- Arrow support ---

class StringComponentType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.utf8(), "rerun.testing.datatypes.StringComponent"
        )

class StringComponentArray(BaseExtensionArray[StringComponentArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.StringComponent"
    _EXTENSION_TYPE = StringComponentType

    @staticmethod
    def _native_to_pa_array(data: StringComponentArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError

StringComponentType._ARRAY_TYPE = StringComponentArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(StringComponentType())


