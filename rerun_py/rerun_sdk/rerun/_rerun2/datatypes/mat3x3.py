# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/mat3x3.fbs".


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
from ._overrides import mat3x3_coeffs__field_converter_override  # noqa: F401

__all__ = ["Mat3x3", "Mat3x3Array", "Mat3x3ArrayLike", "Mat3x3Like", "Mat3x3Type"]


@define
class Mat3x3:
    """A 3x3 column-major Matrix."""

    # You can define your own __init__ function by defining a function called {init_override_name:?}

    coeffs: npt.NDArray[np.float32] = field(converter=mat3x3_coeffs__field_converter_override)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can replace `np.asarray` here with your own code by defining a function named "mat3x3__as_array_override"
        return np.asarray(self.coeffs, dtype=dtype)


if TYPE_CHECKING:
    Mat3x3Like = Union[Mat3x3, Sequence[float], Sequence[Sequence[float]]]
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
        raise NotImplementedError  # You need to implement "mat3x3__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/mat3x3.py


Mat3x3Type._ARRAY_TYPE = Mat3x3Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Mat3x3Type())
