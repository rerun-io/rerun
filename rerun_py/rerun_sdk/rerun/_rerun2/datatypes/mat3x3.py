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
from ._overrides import mat3x3_coeffs_converter  # noqa: F401
__all__ = ["Mat3x3", "Mat3x3Array", "Mat3x3ArrayLike", "Mat3x3Like", "Mat3x3Type"]

@define
class Mat3x3:
    """
    A 3x3 column-major Matrix.
    """

    coeffs: npt.NDArray[np.float32] = field(converter=mat3x3_coeffs_converter)
    def __array__(self, dtype: npt.DTypeLike=None) -> npt.NDArray[Any]:
        return np.asarray(self.coeffs, dtype=dtype)


if TYPE_CHECKING:
    Mat3x3Like = Union[
        Mat3x3,
        Sequence[float], Sequence[Sequence[float]]
    ]
else:
    Mat3x3Like = Any

Mat3x3ArrayLike = Union[
    Mat3x3,
    Sequence[Mat3x3Like],
    
]


# --- Arrow support ---

class Mat3x3Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 9), "rerun.datatypes.Mat3x3"
        )

class Mat3x3Array(BaseExtensionArray[Mat3x3ArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Mat3x3"
    _EXTENSION_TYPE = Mat3x3Type

    @staticmethod
    def _native_to_pa_array(data: Mat3x3ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError

Mat3x3Type._ARRAY_TYPE = Mat3x3Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Mat3x3Type())


