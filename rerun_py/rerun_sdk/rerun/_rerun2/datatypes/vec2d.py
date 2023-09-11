# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/vec2d.fbs".


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
from ._overrides import override_vec2d___native_to_pa_array_override  # noqa: F401

__all__ = ["Vec2D", "Vec2DArray", "Vec2DArrayLike", "Vec2DLike", "Vec2DType"]


@define
class Vec2D:
    """A vector in 2D space."""

    # You can define your own __init__ function by defining a function called {init_override_name:?}

    xy: npt.NDArray[np.float32] = field(converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can replace `np.asarray` here with your own code by defining a function named "override_vec2d__as_array_override"
        return np.asarray(self.xy, dtype=dtype)


if TYPE_CHECKING:
    Vec2DLike = Union[Vec2D, npt.NDArray[Any], npt.ArrayLike, Sequence[float]]
else:
    Vec2DLike = Any

Vec2DArrayLike = Union[
    Vec2D, Sequence[Vec2DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[float]], Sequence[float]
]


# --- Arrow support ---


class Vec2DType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 2), "rerun.datatypes.Vec2D"
        )


class Vec2DArray(BaseExtensionArray[Vec2DArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Vec2D"
    _EXTENSION_TYPE = Vec2DType

    @staticmethod
    def _native_to_pa_array(data: Vec2DArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "override_vec2d__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/vec2d.py


Vec2DType._ARRAY_TYPE = Vec2DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Vec2DType())
