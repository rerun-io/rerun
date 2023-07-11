# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import scale3d_inner_converter  # noqa: F401

__all__ = ["Scale3D", "Scale3DArray", "Scale3DArrayLike", "Scale3DLike", "Scale3DType"]


@define
class Scale3D:
    """
    3D scaling factor, part of a transform representation.

    Example
    -------
    ```python
    # uniform scaling
    scale = rr.dt.Scale3D(3.)

    # non-uniform scaling
    scale = rr.dt.Scale3D([1, 1, -1])
    scale = rr.dt.Scale3D(rr.dt.Vec3D([1, 1, -1]))
    ```
    """

    inner: Union[datatypes.Vec3D, float] = field(converter=scale3d_inner_converter)
    """
    ThreeD (datatypes.Vec3D):
        Individual scaling factors for each axis, distorting the original object.

    Uniform (float):
        Uniform scaling factor along all axis.
    """


if TYPE_CHECKING:
    Scale3DLike = Union[Scale3D, datatypes.Vec3D, float, datatypes.Vec3DLike]
    Scale3DArrayLike = Union[
        Scale3D,
        datatypes.Vec3D,
        float,
        Sequence[Scale3DLike],
    ]
else:
    Scale3DLike = Any
    Scale3DArrayLike = Any


# --- Arrow support ---


class Scale3DType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.dense_union(
                [
                    pa.field("ThreeD", pa.list_(pa.field("item", pa.float32(), False, {}), 3), False, {}),
                    pa.field("Uniform", pa.float32(), False, {}),
                ]
            ),
            "rerun.datatypes.Scale3D",
        )


class Scale3DArray(BaseExtensionArray[Scale3DArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Scale3D"
    _EXTENSION_TYPE = Scale3DType

    @staticmethod
    def _native_to_pa_array(data: Scale3DArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


Scale3DType._ARRAY_TYPE = Scale3DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Scale3DType())
