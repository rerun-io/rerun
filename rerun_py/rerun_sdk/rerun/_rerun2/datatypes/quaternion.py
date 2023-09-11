# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/quaternion.fbs".


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
from .._converters import (
    to_np_float32,
)
from ._overrides import override_quaternion_init  # noqa: F401

__all__ = ["Quaternion", "QuaternionArray", "QuaternionArrayLike", "QuaternionLike", "QuaternionType"]


@define(init=False)
class Quaternion:
    """
    A Quaternion represented by 4 real numbers.

    Note: although the x,y,z,w components of the quaternion will be passed through to the
    datastore as provided, when used in the viewer Quaternions will always be normalized.
    """

    def __init__(self, *args, **kwargs):  # type: ignore[no-untyped-def]
        override_quaternion_init(self, *args, **kwargs)

    xyzw: npt.NDArray[np.float32] = field(converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can replace `np.asarray` here with your own code by defining a function named "override_quaternion_as_array"
        return np.asarray(self.xyzw, dtype=dtype)


QuaternionLike = Quaternion
QuaternionArrayLike = Union[
    Quaternion,
    Sequence[QuaternionLike],
]


# --- Arrow support ---


class QuaternionType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 4), "rerun.datatypes.Quaternion"
        )


class QuaternionArray(BaseExtensionArray[QuaternionArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Quaternion"
    _EXTENSION_TYPE = QuaternionType

    @staticmethod
    def _native_to_pa_array(data: QuaternionArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "override_quaternion_native_to_pa_array" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/quaternion.py


QuaternionType._ARRAY_TYPE = QuaternionArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(QuaternionType())
