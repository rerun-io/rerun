# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa

__all__ = [
    "AffixFuzzer1",
    "AffixFuzzer1Array",
    "AffixFuzzer1ArrayLike",
    "AffixFuzzer1Like",
    "AffixFuzzer1Type",
    "AffixFuzzer2",
    "AffixFuzzer2Array",
    "AffixFuzzer2ArrayLike",
    "AffixFuzzer2Like",
    "AffixFuzzer2Type",
]


## --- AffixFuzzer1 --- ##


@dataclass
class AffixFuzzer1:
    single_string_required: str
    many_strings_required: npt.ArrayLike
    single_float_optional: float | None = None
    single_string_optional: str | None = None
    many_floats_optional: npt.ArrayLike | None = None
    many_strings_optional: npt.ArrayLike | None = None


AffixFuzzer1Like = AffixFuzzer1
AffixFuzzer1ArrayLike = Union[
    AffixFuzzer1Like,
    Sequence[AffixFuzzer1Like],
]


# --- Arrow support ---

from .fuzzy_ext import AffixFuzzer1ArrayExt  # noqa: E402


class AffixFuzzer1Type(pa.ExtensionType):  # type: ignore[misc]
    def __init__(self: type[pa.ExtensionType]) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field("single_float_optional", pa.float32(), True, {}),
                    pa.field("single_string_required", pa.utf8(), False, {}),
                    pa.field("single_string_optional", pa.utf8(), True, {}),
                    pa.field("many_floats_optional", pa.list_(pa.field("item", pa.float32(), True, {})), True, {}),
                    pa.field("many_strings_required", pa.list_(pa.field("item", pa.utf8(), False, {})), False, {}),
                    pa.field("many_strings_optional", pa.list_(pa.field("item", pa.utf8(), True, {})), True, {}),
                ]
            ),
            "rerun.testing.datatypes.AffixFuzzer1",
        )

    def __arrow_ext_serialize__(self: type[pa.ExtensionType]) -> bytes:
        # since we don't have a parameterized type, we don't need extra metadata to be deserialized
        return b""

    @classmethod
    def __arrow_ext_deserialize__(
        cls: type[pa.ExtensionType], storage_type: Any, serialized: Any
    ) -> type[pa.ExtensionType]:
        # return an instance of this subclass given the serialized metadata.
        return AffixFuzzer1Type()

    def __arrow_ext_class__(self: type[pa.ExtensionType]) -> type[pa.ExtensionArray]:
        return AffixFuzzer1Array


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer1Type())


class AffixFuzzer1Array(AffixFuzzer1ArrayExt):  # type: ignore[misc]
    _extension_name = "rerun.testing.datatypes.AffixFuzzer1"

    @staticmethod
    def from_similar(data: AffixFuzzer1ArrayLike | None) -> pa.Array:
        if data is None:
            return AffixFuzzer1Type().wrap_array(pa.array([], type=AffixFuzzer1Type().storage_type))
        else:
            return AffixFuzzer1ArrayExt._from_similar(
                data,
                mono=AffixFuzzer1,
                mono_aliases=AffixFuzzer1Like,
                many=AffixFuzzer1Array,
                many_aliases=AffixFuzzer1ArrayLike,
                arrow=AffixFuzzer1Type,
            )


## --- AffixFuzzer2 --- ##


@dataclass
class AffixFuzzer2:
    single_float_optional: float | None = None

    def __array__(self) -> npt.ArrayLike:
        return np.asarray(self.single_float_optional)


AffixFuzzer2Like = AffixFuzzer2
AffixFuzzer2ArrayLike = Union[
    AffixFuzzer2Like,
    Sequence[AffixFuzzer2Like],
]


# --- Arrow support ---

from .fuzzy_ext import AffixFuzzer2ArrayExt  # noqa: E402


class AffixFuzzer2Type(pa.ExtensionType):  # type: ignore[misc]
    def __init__(self: type[pa.ExtensionType]) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), "rerun.testing.datatypes.AffixFuzzer2")

    def __arrow_ext_serialize__(self: type[pa.ExtensionType]) -> bytes:
        # since we don't have a parameterized type, we don't need extra metadata to be deserialized
        return b""

    @classmethod
    def __arrow_ext_deserialize__(
        cls: type[pa.ExtensionType], storage_type: Any, serialized: Any
    ) -> type[pa.ExtensionType]:
        # return an instance of this subclass given the serialized metadata.
        return AffixFuzzer2Type()

    def __arrow_ext_class__(self: type[pa.ExtensionType]) -> type[pa.ExtensionArray]:
        return AffixFuzzer2Array


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer2Type())


class AffixFuzzer2Array(AffixFuzzer2ArrayExt):  # type: ignore[misc]
    _extension_name = "rerun.testing.datatypes.AffixFuzzer2"

    @staticmethod
    def from_similar(data: AffixFuzzer2ArrayLike | None) -> pa.Array:
        if data is None:
            return AffixFuzzer2Type().wrap_array(pa.array([], type=AffixFuzzer2Type().storage_type))
        else:
            return AffixFuzzer2ArrayExt._from_similar(
                data,
                mono=AffixFuzzer2,
                mono_aliases=AffixFuzzer2Like,
                many=AffixFuzzer2Array,
                many_aliases=AffixFuzzer2ArrayLike,
                arrow=AffixFuzzer2Type,
            )
