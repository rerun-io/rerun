# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/media_type.fbs".

# You can extend this class by creating a "MediaTypeExt" class in "media_type_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["MediaType", "MediaTypeArray", "MediaTypeType"]


class MediaType(datatypes.Utf8):
    """
    [MIME-type](https://en.wikipedia.org/wiki/Media_type) of an entity.

    For instance:
    * `text/plain`
    * `text/markdown`
    """

    # You can define your own __init__ function as a member of MediaTypeExt in media_type_ext.py

    # Note: there are no fields here because MediaType delegates to datatypes.Utf8
    pass


class MediaTypeType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.components.MediaType"
    _DELEGATED_EXTENSION_TYPE = datatypes.Utf8Type


class MediaTypeArray(BaseDelegatingExtensionArray[datatypes.Utf8ArrayLike]):
    _EXTENSION_NAME = "rerun.components.MediaType"
    _EXTENSION_TYPE = MediaTypeType
    _DELEGATED_ARRAY_TYPE = datatypes.Utf8Array


MediaTypeType._ARRAY_TYPE = MediaTypeArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(MediaTypeType())
