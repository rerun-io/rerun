# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa

__all__ = ["Vec2D", "Vec2DArray", "Vec2DArrayLike", "Vec2DLike", "Vec2DType"]


## --- Vec2D --- ##


@dataclass
class Vec2D:
    """A vector in 2D space."""

    xy: npt.ArrayLike

    def __array__(self) -> npt.ArrayLike:
        return np.asarray(self.xy)


Vec2DLike = Vec2D
Vec2DArrayLike = Union[
    Vec2DLike,
    Sequence[Vec2DLike],
]


# --- Arrow support ---

from .vec2d_ext import Vec2DArrayExt  # noqa: E402


class Vec2DType(pa.ExtensionType):  # type: ignore[misc]
    def __init__(self: type[pa.ExtensionType]) -> None:
        pa.ExtensionType.__init__(self, pa.list_(pa.field("item", pa.float32(), False, {}), 2), "rerun.datatypes.Vec2D")

    def __arrow_ext_serialize__(self: type[pa.ExtensionType]) -> bytes:
        # since we don't have a parameterized type, we don't need extra metadata to be deserialized
        return b""

    @classmethod
    def __arrow_ext_deserialize__(
        cls: type[pa.ExtensionType], storage_type: Any, serialized: Any
    ) -> type[pa.ExtensionType]:
        # return an instance of this subclass given the serialized metadata.
        return Vec2DType()

    def __arrow_ext_class__(self: type[pa.ExtensionType]) -> type[pa.ExtensionArray]:
        return Vec2DArray


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Vec2DType())


class Vec2DArray(Vec2DArrayExt):  # type: ignore[misc]
    _extension_name = "rerun.datatypes.Vec2D"

    @staticmethod
    def from_similar(data: Vec2DArrayLike | None) -> pa.Array:
        if data is None:
            return Vec2DType().wrap_array(pa.array([], type=Vec2DType().storage_type))
        else:
            return Vec2DArrayExt._from_similar(
                data,
                mono=Vec2D,
                mono_aliases=Vec2DLike,
                many=Vec2DArray,
                many_aliases=Vec2DArrayLike,
                arrow=Vec2DType,
            )
