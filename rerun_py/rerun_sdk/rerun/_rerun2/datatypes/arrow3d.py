# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from typing import Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import arrow3d_native_to_pa_array  # noqa: F401

__all__ = ["Arrow3D", "Arrow3DArray", "Arrow3DArrayLike", "Arrow3DLike", "Arrow3DType"]


def _arrow3d_origin_converter(x: datatypes.Vec3DLike) -> datatypes.Vec3D:
    if isinstance(x, datatypes.Vec3D):
        return x
    else:
        return datatypes.Vec3D(x)


def _arrow3d_vector_converter(x: datatypes.Vec3DLike) -> datatypes.Vec3D:
    if isinstance(x, datatypes.Vec3D):
        return x
    else:
        return datatypes.Vec3D(x)


@define
class Arrow3D:
    """An arrow in 3D space."""

    origin: datatypes.Vec3D = field(converter=_arrow3d_origin_converter)
    vector: datatypes.Vec3D = field(converter=_arrow3d_vector_converter)


Arrow3DLike = Arrow3D
Arrow3DArrayLike = Union[
    Arrow3D,
    Sequence[Arrow3DLike],
]


# --- Arrow support ---


class Arrow3DType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field("origin", pa.list_(pa.field("item", pa.float32(), False, {}), 3), False, {}),
                    pa.field("vector", pa.list_(pa.field("item", pa.float32(), False, {}), 3), False, {}),
                ]
            ),
            "rerun.datatypes.Arrow3D",
        )


class Arrow3DArray(BaseExtensionArray[Arrow3DArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Arrow3D"
    _EXTENSION_TYPE = Arrow3DType

    @staticmethod
    def _native_to_pa_array(data: Arrow3DArrayLike, data_type: pa.DataType) -> pa.Array:
        return arrow3d_native_to_pa_array(data, data_type)


Arrow3DType._ARRAY_TYPE = Arrow3DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Arrow3DType())
