# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/transform3d.fbs".

# You can extend this class by creating a "Transform3DExt" class in "transform3d_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
)
from .transform3d_ext import Transform3DExt

__all__ = ["Transform3D", "Transform3DArrayLike", "Transform3DBatch", "Transform3DLike", "Transform3DType"]


@define
class Transform3D(Transform3DExt):
    """**Datatype**: Representation of a 3D affine transform."""

    # You can define your own __init__ function as a member of Transform3DExt in transform3d_ext.py

    inner: datatypes.TranslationRotationScale3D = field()
    """
    Must be one of:

    * TranslationRotationScale (datatypes.TranslationRotationScale3D):
        Translation, rotation and scale, decomposed.
    """


if TYPE_CHECKING:
    Transform3DLike = Union[
        Transform3D,
        datatypes.TranslationRotationScale3D,
    ]
    Transform3DArrayLike = Union[
        Transform3D,
        datatypes.TranslationRotationScale3D,
        Sequence[Transform3DLike],
    ]
else:
    Transform3DLike = Any
    Transform3DArrayLike = Any


class Transform3DType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Transform3D"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.dense_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field(
                    "TranslationRotationScale",
                    pa.struct([
                        pa.field(
                            "translation",
                            pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3),
                            nullable=True,
                            metadata={},
                        ),
                        pa.field(
                            "rotation",
                            pa.dense_union([
                                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                                pa.field(
                                    "Quaternion",
                                    pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 4),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "AxisAngle",
                                    pa.struct([
                                        pa.field(
                                            "axis",
                                            pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3),
                                            nullable=False,
                                            metadata={},
                                        ),
                                        pa.field(
                                            "angle",
                                            pa.dense_union([
                                                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                                                pa.field("Radians", pa.float32(), nullable=False, metadata={}),
                                                pa.field("Degrees", pa.float32(), nullable=False, metadata={}),
                                            ]),
                                            nullable=False,
                                            metadata={},
                                        ),
                                    ]),
                                    nullable=False,
                                    metadata={},
                                ),
                            ]),
                            nullable=True,
                            metadata={},
                        ),
                        pa.field(
                            "scale",
                            pa.dense_union([
                                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                                pa.field(
                                    "ThreeD",
                                    pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field("Uniform", pa.float32(), nullable=False, metadata={}),
                            ]),
                            nullable=True,
                            metadata={},
                        ),
                        pa.field("from_parent", pa.bool_(), nullable=False, metadata={}),
                    ]),
                    nullable=False,
                    metadata={},
                ),
            ]),
            self._TYPE_NAME,
        )


class Transform3DBatch(BaseBatch[Transform3DArrayLike]):
    _ARROW_TYPE = Transform3DType()

    @staticmethod
    def _native_to_pa_array(data: Transform3DArrayLike, data_type: pa.DataType) -> pa.Array:
        return Transform3DExt.native_to_pa_array_override(data, data_type)
