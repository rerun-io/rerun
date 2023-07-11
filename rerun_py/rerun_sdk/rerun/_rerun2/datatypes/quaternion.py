# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

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

__all__ = ["Quaternion", "QuaternionArray", "QuaternionArrayLike", "QuaternionLike", "QuaternionType"]


@define
class Quaternion:
    """A Quaternion represented by 4 real numbers."""

    xyzw: npt.NDArray[np.float32] = field(converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        return np.asarray(self.xyzw, dtype=dtype)


if TYPE_CHECKING:
    QuaternionLike = Quaternion

    QuaternionArrayLike = Union[
        Quaternion,
        Sequence[QuaternionLike],
    ]
else:
    QuaternionLike = Any
    QuaternionArrayLike = Any


# --- Arrow support ---


class QuaternionType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), False, {}), 4), "rerun.datatypes.Quaternion"
        )


class QuaternionArray(BaseExtensionArray[QuaternionArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Quaternion"
    _EXTENSION_TYPE = QuaternionType

    @staticmethod
    def _native_to_pa_array(data: QuaternionArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


QuaternionType._ARRAY_TYPE = QuaternionArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(QuaternionType())
