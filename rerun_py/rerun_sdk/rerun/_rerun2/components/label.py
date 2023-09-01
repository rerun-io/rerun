# DO NOT EDIT!: This file was autogenerated by re_types_builder in crates/re_types_builder/src/codegen/python.rs:277

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["LabelArray", "LabelType"]


class LabelType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.label"
    _DELEGATED_EXTENSION_TYPE = datatypes.LabelType


class LabelArray(BaseDelegatingExtensionArray[datatypes.LabelArrayLike]):
    _EXTENSION_NAME = "rerun.label"
    _EXTENSION_TYPE = LabelType
    _DELEGATED_ARRAY_TYPE = datatypes.LabelArray


LabelType._ARRAY_TYPE = LabelArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(LabelType())
