# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/blueprint/components/corner_2d.fbs".

# You can extend this class by creating a "Corner2DExt" class in "corner2d_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from ..._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin

__all__ = ["Corner2D", "Corner2DArrayLike", "Corner2DBatch", "Corner2DLike", "Corner2DType"]


@define(init=False)
class Corner2D:
    """**Component**: One of four 2D corners, typically used to align objects."""

    def __init__(self: Any, location: Corner2DLike):
        """
        Create a new instance of the Corner2D component.

        Parameters
        ----------
        location:
            Where should the legend be located.

            Allowed values:
             - LeftTop = 1,
             - RightTop = 2,
             - LeftBottom = 3,
             - RightBottom = 4

        """

        # You can define your own __init__ function as a member of Corner2DExt in corner2d_ext.py
        self.__attrs_init__(location=location)

    location: int = field(converter=int)
    # Where should the legend be located.
    #
    # Allowed values:
    #  - LeftTop = 1,
    #  - RightTop = 2,
    #  - LeftBottom = 3,
    #  - RightBottom = 4
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of Corner2DExt in corner2d_ext.py
        return np.asarray(self.location, dtype=dtype)

    def __int__(self) -> int:
        return int(self.location)


Corner2DLike = Corner2D
Corner2DArrayLike = Union[
    Corner2D,
    Sequence[Corner2DLike],
]


class Corner2DType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.Corner2D"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class Corner2DBatch(BaseBatch[Corner2DArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = Corner2DType()

    @staticmethod
    def _native_to_pa_array(data: Corner2DArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in corner2d_ext.py
