# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from typing import Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from .._converters import (
    to_np_float32,
)

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
    "FlattenedScalar",
    "FlattenedScalarArray",
    "FlattenedScalarArrayLike",
    "FlattenedScalarLike",
    "FlattenedScalarType",
]


@define
class FlattenedScalar:
    value: float = field()

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.ArrayLike:
        return np.asarray(self.value, dtype=dtype)

    def __float__(self) -> float:
        return float(self.value)


FlattenedScalarLike = FlattenedScalar
FlattenedScalarArrayLike = Union[
    FlattenedScalar,
    Sequence[FlattenedScalarLike],
]


# --- Arrow support ---


class FlattenedScalarType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.struct([pa.field("value", pa.float32(), False, {})]), "rerun.testing.datatypes.FlattenedScalar"
        )


class FlattenedScalarArray(BaseExtensionArray[FlattenedScalarArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.FlattenedScalar"
    _EXTENSION_TYPE = FlattenedScalarType

    @staticmethod
    def _native_to_pa_array(data: FlattenedScalarArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


FlattenedScalarType._ARRAY_TYPE = FlattenedScalarArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(FlattenedScalarType())


@define
class AffixFuzzer1:
    single_float_optional: float = field()
    single_string_required: str = field()
    single_string_optional: str = field()
    many_floats_optional: npt.NDArray[np.float32] = field(converter=to_np_float32)
    many_strings_required: list[str] = field()
    many_strings_optional: list[str] = field()
    flattened_scalar: float = field()
    almost_flattened_scalar: datatypes.FlattenedScalar = field()


AffixFuzzer1Like = AffixFuzzer1
AffixFuzzer1ArrayLike = Union[
    AffixFuzzer1,
    Sequence[AffixFuzzer1Like],
]


# --- Arrow support ---


class AffixFuzzer1Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field("single_float_optional", pa.float32(), False, {}),
                    pa.field("single_string_required", pa.utf8(), False, {}),
                    pa.field("single_string_optional", pa.utf8(), False, {}),
                    pa.field("many_floats_optional", pa.list_(pa.field("item", pa.float32(), False, {})), False, {}),
                    pa.field("many_strings_required", pa.list_(pa.field("item", pa.utf8(), False, {})), False, {}),
                    pa.field("many_strings_optional", pa.list_(pa.field("item", pa.utf8(), False, {})), False, {}),
                    pa.field("flattened_scalar", pa.float32(), False, {}),
                    pa.field(
                        "almost_flattened_scalar", pa.struct([pa.field("value", pa.float32(), False, {})]), False, {}
                    ),
                ]
            ),
            "rerun.testing.datatypes.AffixFuzzer1",
        )


class AffixFuzzer1Array(BaseExtensionArray[AffixFuzzer1ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.AffixFuzzer1"
    _EXTENSION_TYPE = AffixFuzzer1Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer1ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer1Type._ARRAY_TYPE = AffixFuzzer1Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer1Type())


@define
class AffixFuzzer2:
    single_float_optional: float = field()

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.ArrayLike:
        return np.asarray(self.single_float_optional, dtype=dtype)

    def __float__(self) -> float:
        return float(self.single_float_optional)


AffixFuzzer2Like = AffixFuzzer2
AffixFuzzer2ArrayLike = Union[
    AffixFuzzer2,
    Sequence[AffixFuzzer2Like],
]


# --- Arrow support ---


class AffixFuzzer2Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), "rerun.testing.datatypes.AffixFuzzer2")


class AffixFuzzer2Array(BaseExtensionArray[AffixFuzzer2ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.datatypes.AffixFuzzer2"
    _EXTENSION_TYPE = AffixFuzzer2Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer2ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer2Type._ARRAY_TYPE = AffixFuzzer2Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer2Type())
