# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/quaternion.fbs".

# You can extend this class by creating a "QuaternionExt" class in "quaternion_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseBatch,
)
from .._converters import (
    to_np_float32,
)
from .quaternion_ext import QuaternionExt

__all__ = ["Quaternion", "QuaternionArrayLike", "QuaternionBatch", "QuaternionLike"]


@define(init=False)
class Quaternion(QuaternionExt):
    """
    **Datatype**: A Quaternion represented by 4 real numbers.

    Note: although the x,y,z,w components of the quaternion will be passed through to the
    datastore as provided, when used in the Viewer Quaternions will always be normalized.
    """

    # __init__ can be found in quaternion_ext.py

    xyzw: npt.NDArray[np.float32] = field(converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None, copy: bool | None = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of QuaternionExt in quaternion_ext.py
        return np.asarray(self.xyzw, dtype=dtype, copy=copy)


QuaternionLike = Quaternion
QuaternionArrayLike = Union[
    Quaternion, Sequence[QuaternionLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[float]]
]


class QuaternionBatch(BaseBatch[QuaternionArrayLike]):
    _ARROW_DATATYPE = pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 4)

    @staticmethod
    def _native_to_pa_array(data: QuaternionArrayLike, data_type: pa.DataType) -> pa.Array:
        return QuaternionExt.native_to_pa_array_override(data, data_type)
