# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/view_coordinates.fbs".

# You can extend this class by creating a "ViewCoordinatesExt" class in "view_coordinates_ext.py".

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
    to_np_uint8,
)

__all__ = [
    "ViewCoordinates",
    "ViewCoordinatesArray",
    "ViewCoordinatesArrayLike",
    "ViewCoordinatesLike",
    "ViewCoordinatesType",
]


@define
class ViewCoordinates:
    """
    How we interpret the coordinate system of an entity/space.

    For instance: What is "up"? What does the Z axis mean? Is this right-handed or left-handed?

    The follow constants are used to represent the different directions.
     Up = 1
     Down = 2
     Right = 3
     Left = 4
     Forward = 5
     Back = 6
    """

    # You can define your own __init__ function as a member of ViewCoordinatesExt in view_coordinates_ext.py

    coordinates: npt.NDArray[np.uint8] = field(converter=to_np_uint8)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of ViewCoordinatesExt in view_coordinates_ext.py
        return np.asarray(self.coordinates, dtype=dtype)


ViewCoordinatesLike = ViewCoordinates
ViewCoordinatesArrayLike = Union[
    ViewCoordinates,
    Sequence[ViewCoordinatesLike],
]


# --- Arrow support ---


class ViewCoordinatesType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(pa.field("item", pa.uint8(), nullable=False, metadata={}), 3),
            "rerun.components.ViewCoordinates",
        )


class ViewCoordinatesArray(BaseExtensionArray[ViewCoordinatesArrayLike]):
    _EXTENSION_NAME = "rerun.components.ViewCoordinates"
    _EXTENSION_TYPE = ViewCoordinatesType

    @staticmethod
    def _native_to_pa_array(data: ViewCoordinatesArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in view_coordinates_ext.py


ViewCoordinatesType._ARRAY_TYPE = ViewCoordinatesArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ViewCoordinatesType())
