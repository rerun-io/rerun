# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/vec3d.fbs".


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
from ._overrides import override_vec3d___native_to_pa_array_override  # noqa: F401

__all__ = ["Vec3D", "Vec3DArray", "Vec3DArrayLike", "Vec3DLike", "Vec3DType"]


@define
class Vec3D:
    """A vector in 3D space."""

    # You can define your own __init__ function by defining a function called {init_override_name:?}

    xyz: npt.NDArray[np.float32] = field(converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can replace `np.asarray` here with your own code by defining a function named "override_vec3d__as_array_override"
        return np.asarray(self.xyz, dtype=dtype)


if TYPE_CHECKING:
    Vec3DLike = Union[Vec3D, npt.NDArray[Any], npt.ArrayLike, Sequence[float]]
else:
    Vec3DLike = Any

Vec3DArrayLike = Union[
    Vec3D, Sequence[Vec3DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[float]], Sequence[float]
]


# --- Arrow support ---


class Vec3DType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3), "rerun.datatypes.Vec3D"
        )


class Vec3DArray(BaseExtensionArray[Vec3DArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Vec3D"
    _EXTENSION_TYPE = Vec3DType

    @staticmethod
    def _native_to_pa_array(data: Vec3DArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "override_vec3d__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/vec3d.py


Vec3DType._ARRAY_TYPE = Vec3DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Vec3DType())
