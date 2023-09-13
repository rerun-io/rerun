# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/keypoint_pair.fbs".

# You can extend this class by creating a "KeypointPairExt" class in "keypoint_pair_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import keypoint_pair__native_to_pa_array_override  # noqa: F401

__all__ = ["KeypointPair", "KeypointPairArray", "KeypointPairArrayLike", "KeypointPairLike", "KeypointPairType"]


def _keypoint_pair__keypoint0__special_field_converter_override(x: datatypes.KeypointIdLike) -> datatypes.KeypointId:
    if isinstance(x, datatypes.KeypointId):
        return x
    else:
        return datatypes.KeypointId(x)


def _keypoint_pair__keypoint1__special_field_converter_override(x: datatypes.KeypointIdLike) -> datatypes.KeypointId:
    if isinstance(x, datatypes.KeypointId):
        return x
    else:
        return datatypes.KeypointId(x)


@define
class KeypointPair:
    """A connection between two `Keypoints`."""

    # You can define your own __init__ function as a member of KeypointPairExt in keypoint_pair_ext.py

    keypoint0: datatypes.KeypointId = field(converter=_keypoint_pair__keypoint0__special_field_converter_override)
    keypoint1: datatypes.KeypointId = field(converter=_keypoint_pair__keypoint1__special_field_converter_override)


if TYPE_CHECKING:
    KeypointPairLike = Union[KeypointPair, Sequence[datatypes.KeypointIdLike]]
else:
    KeypointPairLike = Any

KeypointPairArrayLike = Union[
    KeypointPair,
    Sequence[KeypointPairLike],
]


# --- Arrow support ---


class KeypointPairType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field("keypoint0", pa.uint16(), nullable=False, metadata={}),
                    pa.field("keypoint1", pa.uint16(), nullable=False, metadata={}),
                ]
            ),
            "rerun.datatypes.KeypointPair",
        )


class KeypointPairArray(BaseExtensionArray[KeypointPairArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.KeypointPair"
    _EXTENSION_TYPE = KeypointPairType

    @staticmethod
    def _native_to_pa_array(data: KeypointPairArrayLike, data_type: pa.DataType) -> pa.Array:
        return keypoint_pair__native_to_pa_array_override(data, data_type)


KeypointPairType._ARRAY_TYPE = KeypointPairArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(KeypointPairType())
