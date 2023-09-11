# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/instance_key.fbs".


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
from ._overrides import override_instance_key___native_to_pa_array_override  # noqa: F401

__all__ = ["InstanceKey", "InstanceKeyArray", "InstanceKeyArrayLike", "InstanceKeyLike", "InstanceKeyType"]


@define
class InstanceKey:
    """A unique numeric identifier for each individual instance within a batch."""

    # You can define your own __init__ function by defining a function called {init_override_name:?}

    value: int = field(converter=int)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can replace `np.asarray` here with your own code by defining a function named "override_instance_key__as_array_override"
        return np.asarray(self.value, dtype=dtype)

    def __int__(self) -> int:
        return int(self.value)


if TYPE_CHECKING:
    InstanceKeyLike = Union[InstanceKey, int]
else:
    InstanceKeyLike = Any

InstanceKeyArrayLike = Union[InstanceKey, Sequence[InstanceKeyLike], int, npt.NDArray[np.uint64]]


# --- Arrow support ---


class InstanceKeyType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint64(), "rerun.instance_key")


class InstanceKeyArray(BaseExtensionArray[InstanceKeyArrayLike]):
    _EXTENSION_NAME = "rerun.instance_key"
    _EXTENSION_TYPE = InstanceKeyType

    @staticmethod
    def _native_to_pa_array(data: InstanceKeyArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "override_instance_key__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/components/_overrides/instance_key.py


InstanceKeyType._ARRAY_TYPE = InstanceKeyArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(InstanceKeyType())
