# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

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
from ._overrides import classid_native_to_pa_array  # noqa: F401

__all__ = ["ClassId", "ClassIdArray", "ClassIdArrayLike", "ClassIdLike", "ClassIdType"]


@define
class ClassId:
    """A 16-bit ID representing a type of semantic class."""

    id: int = field(converter=int)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        return np.asarray(self.id, dtype=dtype)

    def __int__(self) -> int:
        return int(self.id)


ClassIdLike = Union[ClassId, int]

ClassIdArrayLike = Union[
    ClassId,
    Sequence[ClassIdLike],
    int,
    npt.NDArray[np.uint8],
    npt.NDArray[np.uint16],
    npt.NDArray[np.uint32],
    npt.NDArray[np.uint64],
]


# --- Arrow support ---


class ClassIdType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint16(), "rerun.class_id")


class ClassIdArray(BaseExtensionArray[ClassIdArrayLike]):
    _EXTENSION_NAME = "rerun.class_id"
    _EXTENSION_TYPE = ClassIdType

    @staticmethod
    def _native_to_pa_array(data: ClassIdArrayLike, data_type: pa.DataType) -> pa.Array:
        return classid_native_to_pa_array(data, data_type)


ClassIdType._ARRAY_TYPE = ClassIdArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ClassIdType())
