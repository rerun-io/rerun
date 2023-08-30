# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["KeypointIdArray", "KeypointIdType"]


class KeypointIdType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.keypoint_id"
    _DELEGATED_EXTENSION_TYPE = datatypes.KeypointIdType


class KeypointIdArray(BaseDelegatingExtensionArray[datatypes.KeypointIdArrayLike]):
    _EXTENSION_NAME = "rerun.keypoint_id"
    _EXTENSION_TYPE = KeypointIdType
    _DELEGATED_ARRAY_TYPE = datatypes.KeypointIdArray


KeypointIdType._ARRAY_TYPE = KeypointIdArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(KeypointIdType())
