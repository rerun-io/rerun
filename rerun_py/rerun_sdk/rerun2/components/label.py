# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

__all__ = ["Label", "LabelArray", "LabelArrayLike", "LabelLike", "LabelType"]

from dataclasses import dataclass
from typing import Any, Sequence, Union

import pyarrow as pa


@dataclass
class Label:
    """A String label component."""

    value: str

    def __str__(self):
        return self.value


LabelLike = Union[Label, str]

LabelArrayLike = Union[
    LabelLike,
    Sequence[LabelLike],
]


# --- Arrow support ---

from rerun2.components.label_ext import LabelArrayExt  # noqa: E402


class LabelType(pa.ExtensionType):
    def __init__(self: type[pa.ExtensionType]) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), "rerun.components.Label")

    def __arrow_ext_serialize__(self: type[pa.ExtensionType]) -> bytes:
        # since we don't have a parameterized type, we don't need extra metadata to be deserialized
        return b""

    @classmethod
    def __arrow_ext_deserialize__(
        cls: type[pa.ExtensionType], storage_type: Any, serialized: Any
    ) -> type[pa.ExtensionType]:
        # return an instance of this subclass given the serialized metadata.
        return LabelType()

    def __arrow_ext_class__(self: type[pa.ExtensionType]) -> type[pa.ExtensionArray]:
        return LabelArray


pa.register_extension_type(LabelType())


class LabelArray(pa.ExtensionArray, LabelArrayExt):  # type: ignore[misc]
    @staticmethod
    def from_similar(data: LabelArrayLike | None):
        if data is None:
            return LabelType().wrap_array(pa.array([], type=LabelType().storage_type))
        else:
            return LabelArrayExt._from_similar(
                data,
                mono=Label,
                mono_aliases=LabelLike,
                many=LabelArray,
                many_aliases=LabelArrayLike,
                arrow=LabelType,
            )
