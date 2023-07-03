# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from dataclasses import dataclass, field
from typing import Any, Dict, Iterable, Optional, Sequence, Set, Tuple, Union

from .._baseclasses import Component

__all__ = ["InstanceKey", "InstanceKeyArray", "InstanceKeyArrayLike", "InstanceKeyLike", "InstanceKeyType"]


## --- InstanceKey --- ##


@dataclass
class InstanceKey:
    """
    A unique numeric identifier for each individual instance within a batch.
    """

    value: int

    def __array__(self) -> npt.ArrayLike:
        return np.asarray(self.value)


InstanceKeyLike = Union[InstanceKey, int]

InstanceKeyArrayLike = Union[InstanceKeyLike, Sequence[InstanceKeyLike], npt.NDArray[np.uint64]]


# --- Arrow support ---

from .instance_key_ext import InstanceKeyArrayExt  # noqa: E402


class InstanceKeyType(pa.ExtensionType):  # type: ignore[misc]
    def __init__(self: type[pa.ExtensionType]) -> None:
        pa.ExtensionType.__init__(self, pa.uint64(), "rerun.instance_key")

    def __arrow_ext_serialize__(self: type[pa.ExtensionType]) -> bytes:
        # since we don't have a parameterized type, we don't need extra metadata to be deserialized
        return b""

    @classmethod
    def __arrow_ext_deserialize__(
        cls: type[pa.ExtensionType], storage_type: Any, serialized: Any
    ) -> type[pa.ExtensionType]:
        # return an instance of this subclass given the serialized metadata.
        return InstanceKeyType()

    def __arrow_ext_class__(self: type[pa.ExtensionType]) -> type[pa.ExtensionArray]:
        return InstanceKeyArray


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(InstanceKeyType())


class InstanceKeyArray(Component, InstanceKeyArrayExt):  # type: ignore[misc]
    _extension_name = "rerun.instance_key"

    @staticmethod
    def from_similar(data: InstanceKeyArrayLike | None) -> pa.Array:
        if data is None:
            return InstanceKeyType().wrap_array(pa.array([], type=InstanceKeyType().storage_type))
        else:
            return InstanceKeyArrayExt._from_similar(
                data,
                mono=InstanceKey,
                mono_aliases=InstanceKeyLike,
                many=InstanceKeyArray,
                many_aliases=InstanceKeyArrayLike,
                arrow=InstanceKeyType,
            )
