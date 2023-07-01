# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from dataclasses import dataclass, field
from typing import Any, Dict, Iterable, Optional, Sequence, Set, Tuple, Union

from ._base import Component

__all__ = ["Label", "LabelArray", "LabelArrayLike", "LabelLike", "LabelType"]


## --- Label --- ##


@dataclass
class Label(Component):
    """
    A String label component.
    """

    value: str

    def __str__(self) -> str:
        return self.value


LabelLike = Union[Label, str]

LabelArrayLike = Union[
    LabelLike,
    Sequence[LabelLike],
]


# --- Arrow support ---

from .label_ext import LabelArrayExt  # noqa: E402


class LabelType(pa.ExtensionType):  # type: ignore[misc]
    def __init__(self: type[pa.ExtensionType]) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), "rerun.label")

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


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(LabelType())


class LabelArray(Component, LabelArrayExt):  # type: ignore[misc]
    _extension_name = "rerun.label"

    @staticmethod
    def from_similar(data: LabelArrayLike | None) -> pa.Array:
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
