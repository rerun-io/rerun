# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/depth_meter.fbs".


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
from ._overrides import depth_meter__native_to_pa_array_override  # noqa: F401

__all__ = ["DepthMeter", "DepthMeterArray", "DepthMeterArrayLike", "DepthMeterLike", "DepthMeterType"]


@define
class DepthMeter:
    """A component indicating how long a meter is, expressed in native units."""

    # You can define your own __init__ function as a member of DepthMeterExt in depth_meter_ext.py

    value: float = field(converter=float)  # type: ignore[misc]

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of DepthMeterExt in depth_meter_ext.py
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)


if TYPE_CHECKING:
    DepthMeterLike = Union[DepthMeter, float]
else:
    DepthMeterLike = Any

DepthMeterArrayLike = Union[DepthMeter, Sequence[DepthMeterLike], float, npt.NDArray[np.float32]]


# --- Arrow support ---


class DepthMeterType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), "rerun.components.DepthMeter")


class DepthMeterArray(BaseExtensionArray[DepthMeterArrayLike]):
    _EXTENSION_NAME = "rerun.components.DepthMeter"
    _EXTENSION_TYPE = DepthMeterType

    @staticmethod
    def _native_to_pa_array(data: DepthMeterArrayLike, data_type: pa.DataType) -> pa.Array:
        return depth_meter__native_to_pa_array_override(data, data_type)


DepthMeterType._ARRAY_TYPE = DepthMeterArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(DepthMeterType())
