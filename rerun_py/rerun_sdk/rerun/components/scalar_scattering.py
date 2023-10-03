# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/scalar_scattering.fbs".

# You can extend this class by creating a "ScalarScatteringExt" class in "scalar_scattering_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin
from .scalar_scattering_ext import ScalarScatteringExt

__all__ = [
    "ScalarScattering",
    "ScalarScatteringArrayLike",
    "ScalarScatteringBatch",
    "ScalarScatteringLike",
    "ScalarScatteringType",
]


@define(init=False)
class ScalarScattering(ScalarScatteringExt):
    """If true, a scalar will be shown as individual point in a scatter plot."""

    def __init__(self: Any, scattered: ScalarScatteringLike):
        """Create a new instance of the ScalarScattering component."""

        # You can define your own __init__ function as a member of ScalarScatteringExt in scalar_scattering_ext.py
        self.__attrs_init__(scattered=scattered)

    scattered: bool = field(converter=bool)


if TYPE_CHECKING:
    ScalarScatteringLike = Union[ScalarScattering, bool]
else:
    ScalarScatteringLike = Any

ScalarScatteringArrayLike = Union[ScalarScattering, Sequence[ScalarScatteringLike], bool, npt.NDArray[np.bool_]]


class ScalarScatteringType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.ScalarScattering"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.bool_(), self._TYPE_NAME)


class ScalarScatteringBatch(BaseBatch[ScalarScatteringArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = ScalarScatteringType()

    @staticmethod
    def _native_to_pa_array(data: ScalarScatteringArrayLike, data_type: pa.DataType) -> pa.Array:
        return ScalarScatteringExt.native_to_pa_array_override(data, data_type)
