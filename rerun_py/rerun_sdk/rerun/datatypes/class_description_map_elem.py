# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/class_description_map_elem.fbs".

# You can extend this class by creating a "ClassDescriptionMapElemExt" class in "class_description_map_elem_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import BaseBatch, BaseExtensionType
from .class_description_map_elem_ext import ClassDescriptionMapElemExt

__all__ = [
    "ClassDescriptionMapElem",
    "ClassDescriptionMapElemArrayLike",
    "ClassDescriptionMapElemBatch",
    "ClassDescriptionMapElemLike",
    "ClassDescriptionMapElemType",
]


def _class_description_map_elem__class_id__special_field_converter_override(
    x: datatypes.ClassIdLike,
) -> datatypes.ClassId:
    if isinstance(x, datatypes.ClassId):
        return x
    else:
        return datatypes.ClassId(x)


@define
class ClassDescriptionMapElem(ClassDescriptionMapElemExt):
    """
    A helper type for mapping class IDs to class descriptions.

    This is internal to the `AnnotationContext` structure.
    """

    def __init__(self: Any, class_id: datatypes.ClassIdLike, class_description: datatypes.ClassDescriptionLike):
        # You can define your own __init__ function as a member of ClassDescriptionMapElemExt in class_description_map_elem_ext.py
        self.__attrs_init__(class_id=class_id, class_description=class_description)

    class_id: datatypes.ClassId = field(
        converter=_class_description_map_elem__class_id__special_field_converter_override
    )
    class_description: datatypes.ClassDescription = field()


if TYPE_CHECKING:
    ClassDescriptionMapElemLike = Union[ClassDescriptionMapElem, datatypes.ClassDescriptionLike]
else:
    ClassDescriptionMapElemLike = Any

ClassDescriptionMapElemArrayLike = Union[
    ClassDescriptionMapElem,
    Sequence[ClassDescriptionMapElemLike],
]


class ClassDescriptionMapElemType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.ClassDescriptionMapElem"

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
            self._TYPE_NAME,
        )


class ClassDescriptionMapElemBatch(BaseBatch[ClassDescriptionMapElemArrayLike]):
    _ARROW_TYPE = ClassDescriptionMapElemType()

    @staticmethod
    def _native_to_pa_array(data: ClassDescriptionMapElemArrayLike, data_type: pa.DataType) -> pa.Array:
        return ClassDescriptionMapElemExt.native_to_pa_array_override(data, data_type)


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ClassDescriptionMapElemType())


if hasattr(ClassDescriptionMapElemExt, "deferred_patch_class"):
    ClassDescriptionMapElemExt.deferred_patch_class(ClassDescriptionMapElem)
