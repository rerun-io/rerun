# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/annotation_info.fbs".

# You can extend this class by creating a "AnnotationInfoExt" class in "annotation_info_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Tuple, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import BaseBatch, BaseExtensionType
from .annotation_info_ext import AnnotationInfoExt

__all__ = [
    "AnnotationInfo",
    "AnnotationInfoArrayLike",
    "AnnotationInfoBatch",
    "AnnotationInfoLike",
    "AnnotationInfoType",
]


def _annotation_info__label__special_field_converter_override(x: datatypes.Utf8Like | None) -> datatypes.Utf8 | None:
    if x is None:
        return None
    elif isinstance(x, datatypes.Utf8):
        return x
    else:
        return datatypes.Utf8(x)


def _annotation_info__color__special_field_converter_override(x: datatypes.ColorLike | None) -> datatypes.Color | None:
    if x is None:
        return None
    elif isinstance(x, datatypes.Color):
        return x
    else:
        return datatypes.Color(x)


@define
class AnnotationInfo(AnnotationInfoExt):
    """
    Annotation info annotating a class id or key-point id.

    Color and label will be used to annotate entities/keypoints which reference the id.
    The id refers either to a class or key-point id
    """

    def __init__(self: Any, id: int, label: datatypes.Utf8Like | None = None, color: datatypes.ColorLike | None = None):
        """
        Create a new instance of the AnnotationInfo datatype.

        Parameters
        ----------
        id:
             `ClassId` or `KeypointId` to which this annotation info belongs.
        label:
             The label that will be shown in the UI.
        color:
             The color that will be applied to the annotated entity.
        """

        # You can define your own __init__ function as a member of AnnotationInfoExt in annotation_info_ext.py
        self.__attrs_init__(id=id, label=label, color=color)

    id: int = field(converter=int)
    """
    `ClassId` or `KeypointId` to which this annotation info belongs.
    """

    label: datatypes.Utf8 | None = field(
        default=None, converter=_annotation_info__label__special_field_converter_override
    )
    """
    The label that will be shown in the UI.
    """

    color: datatypes.Color | None = field(
        default=None, converter=_annotation_info__color__special_field_converter_override
    )
    """
    The color that will be applied to the annotated entity.
    """


if TYPE_CHECKING:
    AnnotationInfoLike = Union[AnnotationInfo, int, Tuple[int, str], Tuple[int, str, datatypes.ColorLike]]
else:
    AnnotationInfoLike = Any

AnnotationInfoArrayLike = Union[
    AnnotationInfo,
    Sequence[AnnotationInfoLike],
]


class AnnotationInfoType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.AnnotationInfo"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field("id", pa.uint16(), nullable=False, metadata={}),
                    pa.field("label", pa.utf8(), nullable=True, metadata={}),
                    pa.field("color", pa.uint32(), nullable=True, metadata={}),
                ]
            ),
            self._TYPE_NAME,
        )


class AnnotationInfoBatch(BaseBatch[AnnotationInfoArrayLike]):
    _ARROW_TYPE = AnnotationInfoType()

    @staticmethod
    def _native_to_pa_array(data: AnnotationInfoArrayLike, data_type: pa.DataType) -> pa.Array:
        return AnnotationInfoExt.native_to_pa_array_override(data, data_type)


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AnnotationInfoType())


if hasattr(AnnotationInfoExt, "deferred_patch_class"):
    AnnotationInfoExt.deferred_patch_class(AnnotationInfo)
