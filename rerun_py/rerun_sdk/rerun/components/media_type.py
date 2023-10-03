# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/media_type.fbs".

# You can extend this class by creating a "MediaTypeExt" class in "media_type_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import ComponentBatchMixin
from .media_type_ext import MediaTypeExt

__all__ = ["MediaType", "MediaTypeBatch", "MediaTypeType"]


class MediaType(MediaTypeExt, datatypes.Utf8):
    """
    **Component**: A standardized media type (RFC2046, formerly known as MIME types), encoded as a utf8 string.

    The complete reference of officially registered media types is maintained by the IANA and can be
    consulted at <https://www.iana.org/assignments/media-types/media-types.xhtml>.
    """

    # You can define your own __init__ function as a member of MediaTypeExt in media_type_ext.py

    # Note: there are no fields here because MediaType delegates to datatypes.Utf8
    pass


class MediaTypeType(datatypes.Utf8Type):
    _TYPE_NAME: str = "rerun.components.MediaType"


class MediaTypeBatch(datatypes.Utf8Batch, ComponentBatchMixin):
    _ARROW_TYPE = MediaTypeType()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(MediaTypeType())


if hasattr(MediaTypeExt, "deferred_patch_class"):
    MediaTypeExt.deferred_patch_class(MediaType)
