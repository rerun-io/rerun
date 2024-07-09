# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/datatypes/blob.fbs".

# You can extend this class by creating a "BlobExt" class in "blob_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
)
from .._converters import (
    to_np_uint8,
)

__all__ = ["Blob", "BlobArrayLike", "BlobBatch", "BlobLike", "BlobType"]


@define(init=False)
class Blob:
    """**Datatype**: A binary blob of data."""

    def __init__(self: Any, data: BlobLike):
        """Create a new instance of the Blob datatype."""

        # You can define your own __init__ function as a member of BlobExt in blob_ext.py
        self.__attrs_init__(data=data)

    data: npt.NDArray[np.uint8] = field(converter=to_np_uint8)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of BlobExt in blob_ext.py
        return np.asarray(self.data, dtype=dtype)


if TYPE_CHECKING:
    BlobLike = Union[Blob, bytes, npt.NDArray[np.uint8]]
else:
    BlobLike = Any

BlobArrayLike = Union[Blob, Sequence[BlobLike], bytes, npt.NDArray[np.uint8]]


class BlobType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Blob"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.uint8(), nullable=False, metadata={})), self._TYPE_NAME
        )


class BlobBatch(BaseBatch[BlobArrayLike]):
    _ARROW_TYPE = BlobType()

    @staticmethod
    def _native_to_pa_array(data: BlobArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError(
            "Arrow serialization of Blob not implemented: We lack codegen for arrow-serialization of general structs"
        )  # You need to implement native_to_pa_array_override in blob_ext.py
