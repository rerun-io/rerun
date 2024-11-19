# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/angle.fbs".

# You can extend this class by creating a "AngleExt" class in "angle_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseBatch,
)
from .angle_ext import AngleExt

__all__ = ["Angle", "AngleArrayLike", "AngleBatch", "AngleLike"]


@define(init=False)
class Angle(AngleExt):
    """**Datatype**: Angle in radians."""

    # __init__ can be found in angle_ext.py

    radians: float = field(converter=float)
    # Angle in radians. One turn is equal to 2π (or τ) radians.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of AngleExt in angle_ext.py
        return np.asarray(self.radians, dtype=dtype)

    def __float__(self) -> float:
        return float(self.radians)

    def __hash__(self) -> int:
        return hash(self.radians)


if TYPE_CHECKING:
    AngleLike = Union[Angle, float, int]
else:
    AngleLike = Any

AngleArrayLike = Union[Angle, Sequence[AngleLike], npt.ArrayLike, Sequence[float], Sequence[int]]


class AngleBatch(BaseBatch[AngleArrayLike]):
    _ARROW_DATATYPE = pa.float32()

    @staticmethod
    def _native_to_pa_array(data: AngleArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.float32).flatten()
        return pa.array(array, type=data_type)
