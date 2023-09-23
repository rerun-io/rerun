# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/class_description.fbs".

# You can extend this class by creating a "ClassDescriptionExt" class in "class_description_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from .class_description_ext import ClassDescriptionExt

__all__ = [
    "ClassDescription",
    "ClassDescriptionArray",
    "ClassDescriptionArrayLike",
    "ClassDescriptionLike",
    "ClassDescriptionType",
]


@define(init=False)
class ClassDescription(ClassDescriptionExt):
    """
    The description of a semantic Class.

    If an entity is annotated with a corresponding `ClassId`, rerun will use
    the attached `AnnotationInfo` to derive labels and colors.

    Keypoints within an annotation class can similarly be annotated with a
    `KeypointId` in which case we should defer to the label and color for the
    `AnnotationInfo` specifically associated with the Keypoint.

    Keypoints within the class can also be decorated with skeletal edges.
    Keypoint-connections are pairs of `KeypointId`s. If an edge is
    defined, and both keypoints exist within the instance of the class, then the
    keypoints should be connected with an edge. The edge should be labeled and
    colored as described by the class's `AnnotationInfo`.
    """

    # __init__ can be found in class_description_ext.py

    info: datatypes.AnnotationInfo = field(
        converter=ClassDescriptionExt.info__field_converter_override,  # type: ignore[misc]
    )
    """
    The `AnnotationInfo` for the class.
    """

    keypoint_annotations: list[datatypes.AnnotationInfo] = field(
        converter=ClassDescriptionExt.keypoint_annotations__field_converter_override,  # type: ignore[misc]
    )
    """
    The `AnnotationInfo` for all of the keypoints.
    """

    keypoint_connections: list[datatypes.KeypointPair] = field(
        converter=ClassDescriptionExt.keypoint_connections__field_converter_override,  # type: ignore[misc]
    )
    """
    The connections between keypoints.
    """


if TYPE_CHECKING:
    ClassDescriptionLike = Union[ClassDescription, datatypes.AnnotationInfoLike]
else:
    ClassDescriptionLike = Any

ClassDescriptionArrayLike = Union[
    ClassDescription,
    Sequence[ClassDescriptionLike],
]


# --- Arrow support ---


class ClassDescriptionType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field(
                        "info",
                        pa.struct(
                            [
                                pa.field("id", pa.uint16(), nullable=False, metadata={}),
                                pa.field("label", pa.utf8(), nullable=True, metadata={}),
                                pa.field("color", pa.uint32(), nullable=True, metadata={}),
                            ]
                        ),
                        nullable=False,
                        metadata={},
                    ),
                    pa.field(
                        "keypoint_annotations",
                        pa.list_(
                            pa.field(
                                "item",
                                pa.struct(
                                    [
                                        pa.field("id", pa.uint16(), nullable=False, metadata={}),
                                        pa.field("label", pa.utf8(), nullable=True, metadata={}),
                                        pa.field("color", pa.uint32(), nullable=True, metadata={}),
                                    ]
                                ),
                                nullable=False,
                                metadata={},
                            )
                        ),
                        nullable=False,
                        metadata={},
                    ),
                    pa.field(
                        "keypoint_connections",
                        pa.list_(
                            pa.field(
                                "item",
                                pa.struct(
                                    [
                                        pa.field("keypoint0", pa.uint16(), nullable=False, metadata={}),
                                        pa.field("keypoint1", pa.uint16(), nullable=False, metadata={}),
                                    ]
                                ),
                                nullable=False,
                                metadata={},
                            )
                        ),
                        nullable=False,
                        metadata={},
                    ),
                ]
            ),
            "rerun.datatypes.ClassDescription",
        )


class ClassDescriptionArray(BaseExtensionArray[ClassDescriptionArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.ClassDescription"
    _EXTENSION_TYPE = ClassDescriptionType

    @staticmethod
    def _native_to_pa_array(data: ClassDescriptionArrayLike, data_type: pa.DataType) -> pa.Array:
        return ClassDescriptionExt.native_to_pa_array_override(data, data_type)


ClassDescriptionType._ARRAY_TYPE = ClassDescriptionArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ClassDescriptionType())


if hasattr(ClassDescriptionExt, "deferred_patch_class"):
    ClassDescriptionExt.deferred_patch_class(ClassDescription)
