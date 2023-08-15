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
from ._overrides import classdescriptionmapelem_native_to_pa_array  # noqa: F401

__all__ = [
    "ClassDescriptionMapElem",
    "ClassDescriptionMapElemArray",
    "ClassDescriptionMapElemArrayLike",
    "ClassDescriptionMapElemLike",
    "ClassDescriptionMapElemType",
]


def _classdescriptionmapelem_class_id_converter(x: datatypes.ClassIdLike) -> datatypes.ClassId:
    if isinstance(x, datatypes.ClassId):
        return x
    else:
        return datatypes.ClassId(x)


@define
class ClassDescriptionMapElem:
    """
    A helper type for mapping class IDs to class descriptions.

    This is internal to the `AnnotationContext` structure.
    """

    class_id: datatypes.ClassId = field(converter=_classdescriptionmapelem_class_id_converter)
    class_description: datatypes.ClassDescription = field()


if TYPE_CHECKING:
    ClassDescriptionMapElemLike = Union[ClassDescriptionMapElem, datatypes.ClassDescriptionLike]
else:
    ClassDescriptionMapElemLike = Any

ClassDescriptionMapElemArrayLike = Union[
    ClassDescriptionMapElem,
    Sequence[ClassDescriptionMapElemLike],
]


# --- Arrow support ---


class ClassDescriptionMapElemType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field("class_id", pa.uint16(), nullable=False, metadata={}),
                    pa.field(
                        "class_description",
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
                        nullable=False,
                        metadata={},
                    ),
                ]
            ),
            "rerun.datatypes.ClassDescriptionMapElem",
        )


class ClassDescriptionMapElemArray(BaseExtensionArray[ClassDescriptionMapElemArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.ClassDescriptionMapElem"
    _EXTENSION_TYPE = ClassDescriptionMapElemType

    @staticmethod
    def _native_to_pa_array(data: ClassDescriptionMapElemArrayLike, data_type: pa.DataType) -> pa.Array:
        return classdescriptionmapelem_native_to_pa_array(data, data_type)


ClassDescriptionMapElemType._ARRAY_TYPE = ClassDescriptionMapElemArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ClassDescriptionMapElemType())
