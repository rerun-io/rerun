# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/plane3d.fbs".

# You can extend this class by creating a "Plane3DExt" class in "plane3d_ext.py".

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

__all__ = ["Plane3D", "Plane3DArrayLike", "Plane3DBatch", "Plane3DLike"]


@define(init=False)
class Plane3D:
    """
    **Datatype**: An infinite 3D plane represented by a unit normal vector and a distance.

    Any point P on the plane fulfills the equation `dot(xyz, P) - d = 0`, where `xyz` is the plane's normal and `d` the distance.
    This representation is also known as the Hesse normal form.

    Note: although the normal will be passed through to the
    datastore as provided, when used in the Viewer, planes will always be normalized.
    I.e. the plane with xyz = (2, 0, 0), d = 1 is equivalent to xyz = (1, 0, 0), d = 0.5
    """

    def __init__(self: Any, xyzd: Plane3DLike):
        """Create a new instance of the Plane3D datatype."""

        # You can define your own __init__ function as a member of Plane3DExt in plane3d_ext.py
        self.__attrs_init__(xyzd=xyzd)

    xyzd: npt.NDArray[np.float32] = field(converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.NDArray[Any]:
        # You can define your own __array__ function as a member of Plane3DExt in plane3d_ext.py
        return np.asarray(self.xyzd, dtype=dtype)


Plane3DLike = Plane3D
Plane3DArrayLike = Union[Plane3D, Sequence[Plane3DLike], npt.NDArray[Any], npt.ArrayLike, Sequence[Sequence[float]]]


class Plane3DBatch(BaseBatch[Plane3DArrayLike]):
    _ARROW_DATATYPE = pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 4)

    @staticmethod
    def _native_to_pa_array(data: Plane3DArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError(
            "Arrow serialization of Plane3D not implemented: We lack codegen for arrow-serialization of general structs"
        )  # You need to implement native_to_pa_array_override in plane3d_ext.py
