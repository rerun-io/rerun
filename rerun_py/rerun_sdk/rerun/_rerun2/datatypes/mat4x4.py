# DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/python.rs:277.

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
from ._overrides import mat4x4_coeffs_converter  # noqa: F401

__all__ = ["Mat4x4", "Mat4x4Array", "Mat4x4ArrayLike", "Mat4x4Like", "Mat4x4Type"]


@define
class Mat4x4:
    """A 4x4 column-major Matrix."""

    coeffs: npt.NDArray[np.float32] = field(converter=mat4x4_coeffs_converter)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        return np.asarray(self.coeffs, dtype=dtype)


if TYPE_CHECKING:
    Mat4x4Like = Union[Mat4x4, Sequence[float], Sequence[Sequence[float]]]
else:
    Mat4x4Like = Any

Mat4x4ArrayLike = Union[
    Mat4x4,
    Sequence[Mat4x4Like],
]


# --- Arrow support ---


class Mat4x4Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 16), "rerun.datatypes.Mat4x4"
        )


class Mat4x4Array(BaseExtensionArray[Mat4x4ArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Mat4x4"
    _EXTENSION_TYPE = Mat4x4Type

    @staticmethod
    def _native_to_pa_array(data: Mat4x4ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


Mat4x4Type._ARRAY_TYPE = Mat4x4Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Mat4x4Type())
