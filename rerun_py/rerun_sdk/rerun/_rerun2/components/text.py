# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/text.fbs".


from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["TextArray", "TextType"]


class TextType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.components.Text"
    _DELEGATED_EXTENSION_TYPE = datatypes.Utf8Type


class TextArray(BaseDelegatingExtensionArray[datatypes.Utf8ArrayLike]):
    _EXTENSION_NAME = "rerun.components.Text"
    _EXTENSION_TYPE = TextType
    _DELEGATED_ARRAY_TYPE = datatypes.Utf8Array


TextType._ARRAY_TYPE = TextArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(TextType())
