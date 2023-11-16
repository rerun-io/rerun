# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/uuid.fbs".

# You can extend this class by creating a "UuidExt" class in "uuid_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType
from .._converters import (
    to_np_uint8,
)

__all__ = ["Uuid", "UuidArrayLike", "UuidBatch", "UuidLike", "UuidType"]


@define(init=False)
class Uuid:
    """**Datatype**: A 16-byte uuid."""

    def __init__(self: Any, bytes: UuidLike):
        """Create a new instance of the Uuid datatype."""

        # You can define your own __init__ function as a member of UuidExt in uuid_ext.py
        self.__attrs_init__(bytes=bytes)

    bytes: npt.NDArray[np.uint8] = field(converter=to_np_uint8)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of UuidExt in uuid_ext.py
        return np.asarray(self.bytes, dtype=dtype)


UuidLike = Uuid
UuidArrayLike = Union[
    Uuid,
    Sequence[UuidLike],
]


class UuidType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Uuid"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.uint8(), nullable=False, metadata={}), 16), self._TYPE_NAME
        )


class UuidBatch(BaseBatch[UuidArrayLike]):
    _ARROW_TYPE = UuidType()

    @staticmethod
    def _native_to_pa_array(data: UuidArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in uuid_ext.py
