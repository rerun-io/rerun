# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/vec4d.fbs".


from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from .._converters import (
    to_np_float32,
)
from ._overrides import override_vec4d_native_to_pa_array  # noqa: F401

__all__ = ["Vec4D", "Vec4DArray", "Vec4DArrayLike", "Vec4DLike", "Vec4DType"]


@define
class Vec4D:
    """A vector in 4D space."""

    # You can define your own __init__ function by defining a function called {init_override_name:?}

    xyzw: npt.NDArray[np.float32] = field(converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can replace `np.asarray` here with your own code by defining a function named "override_vec4d_as_array"
        return np.asarray(self.xyzw, dtype=dtype)


if TYPE_CHECKING:
    Vec4DLike = Union[Vec4D, npt.NDArray[Any], npt.ArrayLike, Sequence[float]]
else:
    Vec4DLike = Any

Vec4DArrayLike = Union[
    Vec4D, Sequence[Vec4DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[float]], Sequence[float]
]


# --- Arrow support ---


class Vec4DType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 4), "rerun.datatypes.Vec4D"
        )


class Vec4DArray(BaseExtensionArray[Vec4DArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Vec4D"
    _EXTENSION_TYPE = Vec4DType

    @staticmethod
    def _native_to_pa_array(data: Vec4DArrayLike, data_type: pa.DataType) -> pa.Array:
        return override_vec4d_native_to_pa_array(data, data_type)


Vec4DType._ARRAY_TYPE = Vec4DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Vec4DType())
