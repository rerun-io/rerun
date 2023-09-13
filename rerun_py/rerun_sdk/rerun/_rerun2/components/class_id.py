# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/class_id.fbs".


from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["ClassIdArray", "ClassIdType"]


class ClassIdType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.components.ClassId"
    _DELEGATED_EXTENSION_TYPE = datatypes.ClassIdType


class ClassIdArray(BaseDelegatingExtensionArray[datatypes.ClassIdArrayLike]):
    _EXTENSION_NAME = "rerun.components.ClassId"
    _EXTENSION_TYPE = ClassIdType
    _DELEGATED_ARRAY_TYPE = datatypes.ClassIdArray


ClassIdType._ARRAY_TYPE = ClassIdArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ClassIdType())
