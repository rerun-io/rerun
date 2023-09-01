# DO NOT EDIT!: This file was autogenerated by re_types_builder in crates/re_types_builder/src/codegen/python.rs:277

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import transform3d_native_to_pa_array  # noqa: F401

__all__ = ["Transform3D", "Transform3DArray", "Transform3DArrayLike", "Transform3DLike", "Transform3DType"]


@define
class Transform3D:
    """Representation of a 3D affine transform."""

    inner: datatypes.TranslationAndMat3x3 | datatypes.TranslationRotationScale3D = field()
    """
    TranslationAndMat3x3 (datatypes.TranslationAndMat3x3):

    TranslationRotationScale (datatypes.TranslationRotationScale3D):
    """


if TYPE_CHECKING:
    Transform3DLike = Union[
        Transform3D,
        datatypes.TranslationAndMat3x3,
        datatypes.TranslationRotationScale3D,
    ]
    Transform3DArrayLike = Union[
        Transform3D,
        datatypes.TranslationAndMat3x3,
        datatypes.TranslationRotationScale3D,
        Sequence[Transform3DLike],
    ]
else:
    Transform3DLike = Any
    Transform3DArrayLike = Any

# --- Arrow support ---


class Transform3DType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.dense_union(
                [
                    pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                    pa.field(
                        "TranslationAndMat3x3",
                        pa.struct(
                            [
                                pa.field(
                                    "translation",
                                    pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3),
                                    nullable=True,
                                    metadata={},
                                ),
                                pa.field(
                                    "matrix",
                                    pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 9),
                                    nullable=True,
                                    metadata={},
                                ),
                                pa.field("from_parent", pa.bool_(), nullable=False, metadata={}),
                            ]
                        ),
                        nullable=False,
                        metadata={},
                    ),
                    pa.field(
                        "TranslationRotationScale",
                        pa.struct(
                            [
                                pa.field(
                                    "translation",
                                    pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={}), 3),
                                    nullable=True,
                                    metadata={},
                                ),
                                pa.field(
                                    "rotation",
                                    pa.dense_union(
                                        [
                                            pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                                            pa.field(
                                                "Quaternion",
                                                pa.list_(
                                                    pa.field("item", pa.float32(), nullable=False, metadata={}), 4
                                                ),
                                                nullable=False,
                                                metadata={},
                                            ),
                                            pa.field(
                                                "AxisAngle",
                                                pa.struct(
                                                    [
                                                        pa.field(
                                                            "axis",
                                                            pa.list_(
                                                                pa.field(
                                                                    "item", pa.float32(), nullable=False, metadata={}
                                                                ),
                                                                3,
                                                            ),
                                                            nullable=False,
                                                            metadata={},
                                                        ),
                                                        pa.field(
                                                            "angle",
                                                            pa.dense_union(
                                                                [
                                                                    pa.field(
                                                                        "_null_markers",
                                                                        pa.null(),
                                                                        nullable=True,
                                                                        metadata={},
                                                                    ),
                                                                    pa.field(
                                                                        "Radians",
                                                                        pa.float32(),
                                                                        nullable=False,
                                                                        metadata={},
                                                                    ),
                                                                    pa.field(
                                                                        "Degrees",
                                                                        pa.float32(),
                                                                        nullable=False,
                                                                        metadata={},
                                                                    ),
                                                                ]
                                                            ),
                                                            nullable=False,
                                                            metadata={},
                                                        ),
                                                    ]
                                                ),
                                                nullable=False,
                                                metadata={},
                                            ),
                                        ]
                                    ),
                                    nullable=True,
                                    metadata={},
                                ),
                                pa.field(
                                    "scale",
                                    pa.dense_union(
                                        [
                                            pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                                            pa.field(
                                                "ThreeD",
                                                pa.list_(
                                                    pa.field("item", pa.float32(), nullable=False, metadata={}), 3
                                                ),
                                                nullable=False,
                                                metadata={},
                                            ),
                                            pa.field("Uniform", pa.float32(), nullable=False, metadata={}),
                                        ]
                                    ),
                                    nullable=True,
                                    metadata={},
                                ),
                                pa.field("from_parent", pa.bool_(), nullable=False, metadata={}),
                            ]
                        ),
                        nullable=False,
                        metadata={},
                    ),
                ]
            ),
            "rerun.datatypes.Transform3D",
        )


class Transform3DArray(BaseExtensionArray[Transform3DArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Transform3D"
    _EXTENSION_TYPE = Transform3DType

    @staticmethod
    def _native_to_pa_array(data: Transform3DArrayLike, data_type: pa.DataType) -> pa.Array:
        return transform3d_native_to_pa_array(data, data_type)


Transform3DType._ARRAY_TYPE = Transform3DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Transform3DType())
