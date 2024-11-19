# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/media_type.fbs".

# You can extend this class by creating a "MediaTypeExt" class in "media_type_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)
from .media_type_ext import MediaTypeExt

__all__ = ["MediaType", "MediaTypeBatch"]


class MediaType(MediaTypeExt, datatypes.Utf8, ComponentMixin):
    """
    **Component**: A standardized media type (RFC2046, formerly known as MIME types), encoded as a string.

    The complete reference of officially registered media types is maintained by the IANA and can be
    consulted at <https://www.iana.org/assignments/media-types/media-types.xhtml>.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of MediaTypeExt in media_type_ext.py

    # Note: there are no fields here because MediaType delegates to datatypes.Utf8
    pass


class MediaTypeBatch(datatypes.Utf8Batch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.components.MediaType"


# This is patched in late to avoid circular dependencies.
MediaType._BATCH_TYPE = MediaTypeBatch  # type: ignore[assignment]

MediaTypeExt.deferred_patch_class(MediaType)
