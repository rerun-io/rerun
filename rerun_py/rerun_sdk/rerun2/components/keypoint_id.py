# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

__all__ = ["KeypointId", "KeypointIdArray", "KeypointIdArrayLike", "KeypointIdLike", "KeypointIdType"]

from dataclasses import dataclass
from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa


@dataclass
class KeypointId:
    """
    A 16-bit ID representing a type of semantic keypoint within a class.

    `KeypointId`s are only meaningful within the context of a [`rerun.components.ClassDescription`][].

    Used to look up an [`rerun.components.AnnotationInfo`][] for a Keypoint within the
    [`rerun.components.AnnotationContext`].
    """

    id: int

    def __array__(self) -> npt.ArrayLike:
        return np.asarray(self.id)


KeypointIdLike = Union[KeypointId, float]

KeypointIdArrayLike = Union[
    KeypointIdLike, Sequence[KeypointIdLike], npt.NDArray[np.uint8], npt.NDArray[np.uint16], npt.NDArray[np.uint32]
]


# --- Arrow support ---

from rerun2.components.keypoint_id_ext import KeypointIdArrayExt  # noqa: E402


class KeypointIdType(pa.ExtensionType):  # type: ignore[misc]
    def __init__(self: type[pa.ExtensionType]) -> None:
        pa.ExtensionType.__init__(self, pa.uint16(), "rerun.components.KeypointId")

    def __arrow_ext_serialize__(self: type[pa.ExtensionType]) -> bytes:
        # since we don't have a parameterized type, we don't need extra metadata to be deserialized
        return b""

    @classmethod
    def __arrow_ext_deserialize__(
        cls: type[pa.ExtensionType], storage_type: Any, serialized: Any
    ) -> type[pa.ExtensionType]:
        # return an instance of this subclass given the serialized metadata.
        return KeypointIdType()

    def __arrow_ext_class__(self: type[pa.ExtensionType]) -> type[pa.ExtensionArray]:
        return KeypointIdArray


pa.register_extension_type(KeypointIdType())


class KeypointIdArray(pa.ExtensionArray, KeypointIdArrayExt):  # type: ignore[misc]
    @staticmethod
    def from_similar(data: KeypointIdArrayLike | None) -> pa.Array:
        if data is None:
            return KeypointIdType().wrap_array(pa.array([], type=KeypointIdType().storage_type))
        else:
            return KeypointIdArrayExt._from_similar(
                data,
                mono=KeypointId,
                mono_aliases=KeypointIdLike,
                many=KeypointIdArray,
                many_aliases=KeypointIdArrayLike,
                arrow=KeypointIdType,
            )
