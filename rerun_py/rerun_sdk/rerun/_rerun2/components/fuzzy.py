# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from typing import Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
    BaseExtensionArray,
    BaseExtensionType,
)
from .._converters import (
    to_np_float32,
)

__all__ = [
    "AffixFuzzer10",
    "AffixFuzzer10Array",
    "AffixFuzzer10ArrayLike",
    "AffixFuzzer10Like",
    "AffixFuzzer10Type",
    "AffixFuzzer11",
    "AffixFuzzer11Array",
    "AffixFuzzer11ArrayLike",
    "AffixFuzzer11Like",
    "AffixFuzzer11Type",
    "AffixFuzzer12",
    "AffixFuzzer12Array",
    "AffixFuzzer12ArrayLike",
    "AffixFuzzer12Like",
    "AffixFuzzer12Type",
    "AffixFuzzer13",
    "AffixFuzzer13Array",
    "AffixFuzzer13ArrayLike",
    "AffixFuzzer13Like",
    "AffixFuzzer13Type",
    "AffixFuzzer14Array",
    "AffixFuzzer14Type",
    "AffixFuzzer16",
    "AffixFuzzer16Array",
    "AffixFuzzer16ArrayLike",
    "AffixFuzzer16Like",
    "AffixFuzzer16Type",
    "AffixFuzzer17",
    "AffixFuzzer17Array",
    "AffixFuzzer17ArrayLike",
    "AffixFuzzer17Like",
    "AffixFuzzer17Type",
    "AffixFuzzer18",
    "AffixFuzzer18Array",
    "AffixFuzzer18ArrayLike",
    "AffixFuzzer18Like",
    "AffixFuzzer18Type",
    "AffixFuzzer1Array",
    "AffixFuzzer1Type",
    "AffixFuzzer2Array",
    "AffixFuzzer2Type",
    "AffixFuzzer3Array",
    "AffixFuzzer3Type",
    "AffixFuzzer4Array",
    "AffixFuzzer4Type",
    "AffixFuzzer5Array",
    "AffixFuzzer5Type",
    "AffixFuzzer6Array",
    "AffixFuzzer6Type",
    "AffixFuzzer7",
    "AffixFuzzer7Array",
    "AffixFuzzer7ArrayLike",
    "AffixFuzzer7Like",
    "AffixFuzzer7Type",
    "AffixFuzzer8",
    "AffixFuzzer8Array",
    "AffixFuzzer8ArrayLike",
    "AffixFuzzer8Like",
    "AffixFuzzer8Type",
    "AffixFuzzer9",
    "AffixFuzzer9Array",
    "AffixFuzzer9ArrayLike",
    "AffixFuzzer9Like",
    "AffixFuzzer9Type",
]


class AffixFuzzer1Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer1"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer1Type


class AffixFuzzer1Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer1ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer1"
    _EXTENSION_TYPE = AffixFuzzer1Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer1Array


AffixFuzzer1Type._ARRAY_TYPE = AffixFuzzer1Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer1Type())


class AffixFuzzer2Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer2"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer1Type


class AffixFuzzer2Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer1ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer2"
    _EXTENSION_TYPE = AffixFuzzer2Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer1Array


AffixFuzzer2Type._ARRAY_TYPE = AffixFuzzer2Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer2Type())


class AffixFuzzer3Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer3"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer1Type


class AffixFuzzer3Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer1ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer3"
    _EXTENSION_TYPE = AffixFuzzer3Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer1Array


AffixFuzzer3Type._ARRAY_TYPE = AffixFuzzer3Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer3Type())


class AffixFuzzer4Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer4"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer1Type


class AffixFuzzer4Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer1ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer4"
    _EXTENSION_TYPE = AffixFuzzer4Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer1Array


AffixFuzzer4Type._ARRAY_TYPE = AffixFuzzer4Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer4Type())


class AffixFuzzer5Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer5"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer1Type


class AffixFuzzer5Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer1ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer5"
    _EXTENSION_TYPE = AffixFuzzer5Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer1Array


AffixFuzzer5Type._ARRAY_TYPE = AffixFuzzer5Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer5Type())


class AffixFuzzer6Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer6"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer1Type


class AffixFuzzer6Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer1ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer6"
    _EXTENSION_TYPE = AffixFuzzer6Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer1Array


AffixFuzzer6Type._ARRAY_TYPE = AffixFuzzer6Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer6Type())


@define
class AffixFuzzer7:
    many_optional: list[datatypes.AffixFuzzer1] | None = field(default=None)


AffixFuzzer7Like = AffixFuzzer7
AffixFuzzer7ArrayLike = Union[
    AffixFuzzer7,
    Sequence[AffixFuzzer7Like],
]


# --- Arrow support ---


class AffixFuzzer7Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
                    pa.struct(
                        [
                            pa.field("single_float_optional", pa.float32(), True, {}),
                            pa.field("single_string_required", pa.utf8(), False, {}),
                            pa.field("single_string_optional", pa.utf8(), True, {}),
                            pa.field(
                                "many_floats_optional", pa.list_(pa.field("item", pa.float32(), True, {})), True, {}
                            ),
                            pa.field(
                                "many_strings_required", pa.list_(pa.field("item", pa.utf8(), False, {})), False, {}
                            ),
                            pa.field(
                                "many_strings_optional", pa.list_(pa.field("item", pa.utf8(), True, {})), True, {}
                            ),
                            pa.field("flattened_scalar", pa.float32(), False, {}),
                            pa.field(
                                "almost_flattened_scalar",
                                pa.struct([pa.field("value", pa.float32(), False, {})]),
                                False,
                                {},
                            ),
                        ]
                    ),
                    True,
                    {},
                )
            ),
            "rerun.testing.components.AffixFuzzer7",
        )


class AffixFuzzer7Array(BaseExtensionArray[AffixFuzzer7ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer7"
    _EXTENSION_TYPE = AffixFuzzer7Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer7ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer7Type._ARRAY_TYPE = AffixFuzzer7Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer7Type())


@define
class AffixFuzzer8:
    single_float_optional: float | None = field(default=None)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.ArrayLike:
        return np.asarray(self.single_float_optional, dtype=dtype)


AffixFuzzer8Like = AffixFuzzer8
AffixFuzzer8ArrayLike = Union[
    AffixFuzzer8,
    Sequence[AffixFuzzer8Like],
]


# --- Arrow support ---


class AffixFuzzer8Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.float32(), "rerun.testing.components.AffixFuzzer8")


class AffixFuzzer8Array(BaseExtensionArray[AffixFuzzer8ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer8"
    _EXTENSION_TYPE = AffixFuzzer8Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer8ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer8Type._ARRAY_TYPE = AffixFuzzer8Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer8Type())


@define
class AffixFuzzer9:
    single_string_required: str = field()

    def __str__(self) -> str:
        return str(self.single_string_required)


AffixFuzzer9Like = AffixFuzzer9
AffixFuzzer9ArrayLike = Union[
    AffixFuzzer9,
    Sequence[AffixFuzzer9Like],
]


# --- Arrow support ---


class AffixFuzzer9Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), "rerun.testing.components.AffixFuzzer9")


class AffixFuzzer9Array(BaseExtensionArray[AffixFuzzer9ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer9"
    _EXTENSION_TYPE = AffixFuzzer9Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer9ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer9Type._ARRAY_TYPE = AffixFuzzer9Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer9Type())


@define
class AffixFuzzer10:
    single_string_optional: str | None = field(default=None)


AffixFuzzer10Like = AffixFuzzer10
AffixFuzzer10ArrayLike = Union[
    AffixFuzzer10,
    Sequence[AffixFuzzer10Like],
]


# --- Arrow support ---


class AffixFuzzer10Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), "rerun.testing.components.AffixFuzzer10")


class AffixFuzzer10Array(BaseExtensionArray[AffixFuzzer10ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer10"
    _EXTENSION_TYPE = AffixFuzzer10Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer10ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer10Type._ARRAY_TYPE = AffixFuzzer10Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer10Type())


@define
class AffixFuzzer11:
    many_floats_optional: npt.NDArray[np.float32] | None = field(default=None, converter=to_np_float32)

    def __array__(self, dtype: npt.DTypeLike = None) -> npt.ArrayLike:
        return np.asarray(self.many_floats_optional, dtype=dtype)


AffixFuzzer11Like = AffixFuzzer11
AffixFuzzer11ArrayLike = Union[
    AffixFuzzer11,
    Sequence[AffixFuzzer11Like],
]


# --- Arrow support ---


class AffixFuzzer11Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.float32(), True, {})), "rerun.testing.components.AffixFuzzer11"
        )


class AffixFuzzer11Array(BaseExtensionArray[AffixFuzzer11ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer11"
    _EXTENSION_TYPE = AffixFuzzer11Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer11ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer11Type._ARRAY_TYPE = AffixFuzzer11Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer11Type())


@define
class AffixFuzzer12:
    many_strings_required: list[str] = field()


AffixFuzzer12Like = AffixFuzzer12
AffixFuzzer12ArrayLike = Union[
    AffixFuzzer12,
    Sequence[AffixFuzzer12Like],
]


# --- Arrow support ---


class AffixFuzzer12Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.utf8(), False, {})), "rerun.testing.components.AffixFuzzer12"
        )


class AffixFuzzer12Array(BaseExtensionArray[AffixFuzzer12ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer12"
    _EXTENSION_TYPE = AffixFuzzer12Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer12ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer12Type._ARRAY_TYPE = AffixFuzzer12Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer12Type())


@define
class AffixFuzzer13:
    many_strings_optional: list[str] | None = field(default=None)


AffixFuzzer13Like = AffixFuzzer13
AffixFuzzer13ArrayLike = Union[
    AffixFuzzer13,
    Sequence[AffixFuzzer13Like],
]


# --- Arrow support ---


class AffixFuzzer13Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self, pa.list_(pa.field("item", pa.utf8(), True, {})), "rerun.testing.components.AffixFuzzer13"
        )


class AffixFuzzer13Array(BaseExtensionArray[AffixFuzzer13ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer13"
    _EXTENSION_TYPE = AffixFuzzer13Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer13ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer13Type._ARRAY_TYPE = AffixFuzzer13Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer13Type())


class AffixFuzzer14Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer14"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer3Type


class AffixFuzzer14Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer3ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer14"
    _EXTENSION_TYPE = AffixFuzzer14Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer3Array


AffixFuzzer14Type._ARRAY_TYPE = AffixFuzzer14Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer14Type())


@define
class AffixFuzzer16:
    many_required_unions: list[datatypes.AffixFuzzer3] = field()


AffixFuzzer16Like = AffixFuzzer16
AffixFuzzer16ArrayLike = Union[
    AffixFuzzer16,
    Sequence[AffixFuzzer16Like],
]


# --- Arrow support ---


class AffixFuzzer16Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
                    pa.dense_union(
                        [
                            pa.field("degrees", pa.float32(), False, {}),
                            pa.field("radians", pa.float32(), False, {}),
                            pa.field(
                                "craziness",
                                pa.list_(
                                    pa.field(
                                        "item",
                                        pa.struct(
                                            [
                                                pa.field("single_float_optional", pa.float32(), True, {}),
                                                pa.field("single_string_required", pa.utf8(), False, {}),
                                                pa.field("single_string_optional", pa.utf8(), True, {}),
                                                pa.field(
                                                    "many_floats_optional",
                                                    pa.list_(pa.field("item", pa.float32(), True, {})),
                                                    True,
                                                    {},
                                                ),
                                                pa.field(
                                                    "many_strings_required",
                                                    pa.list_(pa.field("item", pa.utf8(), False, {})),
                                                    False,
                                                    {},
                                                ),
                                                pa.field(
                                                    "many_strings_optional",
                                                    pa.list_(pa.field("item", pa.utf8(), True, {})),
                                                    True,
                                                    {},
                                                ),
                                                pa.field("flattened_scalar", pa.float32(), False, {}),
                                                pa.field(
                                                    "almost_flattened_scalar",
                                                    pa.struct([pa.field("value", pa.float32(), False, {})]),
                                                    False,
                                                    {},
                                                ),
                                            ]
                                        ),
                                        False,
                                        {},
                                    )
                                ),
                                False,
                                {},
                            ),
                        ]
                    ),
                    False,
                    {},
                )
            ),
            "rerun.testing.components.AffixFuzzer16",
        )


class AffixFuzzer16Array(BaseExtensionArray[AffixFuzzer16ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer16"
    _EXTENSION_TYPE = AffixFuzzer16Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer16ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer16Type._ARRAY_TYPE = AffixFuzzer16Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer16Type())


@define
class AffixFuzzer17:
    many_optional_unions: list[datatypes.AffixFuzzer3] | None = field(default=None)


AffixFuzzer17Like = AffixFuzzer17
AffixFuzzer17ArrayLike = Union[
    AffixFuzzer17,
    Sequence[AffixFuzzer17Like],
]


# --- Arrow support ---


class AffixFuzzer17Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
                    pa.dense_union(
                        [
                            pa.field("degrees", pa.float32(), False, {}),
                            pa.field("radians", pa.float32(), False, {}),
                            pa.field(
                                "craziness",
                                pa.list_(
                                    pa.field(
                                        "item",
                                        pa.struct(
                                            [
                                                pa.field("single_float_optional", pa.float32(), True, {}),
                                                pa.field("single_string_required", pa.utf8(), False, {}),
                                                pa.field("single_string_optional", pa.utf8(), True, {}),
                                                pa.field(
                                                    "many_floats_optional",
                                                    pa.list_(pa.field("item", pa.float32(), True, {})),
                                                    True,
                                                    {},
                                                ),
                                                pa.field(
                                                    "many_strings_required",
                                                    pa.list_(pa.field("item", pa.utf8(), False, {})),
                                                    False,
                                                    {},
                                                ),
                                                pa.field(
                                                    "many_strings_optional",
                                                    pa.list_(pa.field("item", pa.utf8(), True, {})),
                                                    True,
                                                    {},
                                                ),
                                                pa.field("flattened_scalar", pa.float32(), False, {}),
                                                pa.field(
                                                    "almost_flattened_scalar",
                                                    pa.struct([pa.field("value", pa.float32(), False, {})]),
                                                    False,
                                                    {},
                                                ),
                                            ]
                                        ),
                                        False,
                                        {},
                                    )
                                ),
                                False,
                                {},
                            ),
                        ]
                    ),
                    True,
                    {},
                )
            ),
            "rerun.testing.components.AffixFuzzer17",
        )


class AffixFuzzer17Array(BaseExtensionArray[AffixFuzzer17ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer17"
    _EXTENSION_TYPE = AffixFuzzer17Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer17ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer17Type._ARRAY_TYPE = AffixFuzzer17Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer17Type())


@define
class AffixFuzzer18:
    many_optional_unions: list[datatypes.AffixFuzzer4] | None = field(default=None)


AffixFuzzer18Like = AffixFuzzer18
AffixFuzzer18ArrayLike = Union[
    AffixFuzzer18,
    Sequence[AffixFuzzer18Like],
]


# --- Arrow support ---


class AffixFuzzer18Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
                    pa.dense_union(
                        [
                            pa.field(
                                "single_required",
                                pa.dense_union(
                                    [
                                        pa.field("degrees", pa.float32(), False, {}),
                                        pa.field("radians", pa.float32(), False, {}),
                                        pa.field(
                                            "craziness",
                                            pa.list_(
                                                pa.field(
                                                    "item",
                                                    pa.struct(
                                                        [
                                                            pa.field("single_float_optional", pa.float32(), True, {}),
                                                            pa.field("single_string_required", pa.utf8(), False, {}),
                                                            pa.field("single_string_optional", pa.utf8(), True, {}),
                                                            pa.field(
                                                                "many_floats_optional",
                                                                pa.list_(pa.field("item", pa.float32(), True, {})),
                                                                True,
                                                                {},
                                                            ),
                                                            pa.field(
                                                                "many_strings_required",
                                                                pa.list_(pa.field("item", pa.utf8(), False, {})),
                                                                False,
                                                                {},
                                                            ),
                                                            pa.field(
                                                                "many_strings_optional",
                                                                pa.list_(pa.field("item", pa.utf8(), True, {})),
                                                                True,
                                                                {},
                                                            ),
                                                            pa.field("flattened_scalar", pa.float32(), False, {}),
                                                            pa.field(
                                                                "almost_flattened_scalar",
                                                                pa.struct([pa.field("value", pa.float32(), False, {})]),
                                                                False,
                                                                {},
                                                            ),
                                                        ]
                                                    ),
                                                    False,
                                                    {},
                                                )
                                            ),
                                            False,
                                            {},
                                        ),
                                    ]
                                ),
                                False,
                                {},
                            ),
                            pa.field(
                                "many_required",
                                pa.list_(
                                    pa.field(
                                        "item",
                                        pa.dense_union(
                                            [
                                                pa.field("degrees", pa.float32(), False, {}),
                                                pa.field("radians", pa.float32(), False, {}),
                                                pa.field(
                                                    "craziness",
                                                    pa.list_(
                                                        pa.field(
                                                            "item",
                                                            pa.struct(
                                                                [
                                                                    pa.field(
                                                                        "single_float_optional", pa.float32(), True, {}
                                                                    ),
                                                                    pa.field(
                                                                        "single_string_required", pa.utf8(), False, {}
                                                                    ),
                                                                    pa.field(
                                                                        "single_string_optional", pa.utf8(), True, {}
                                                                    ),
                                                                    pa.field(
                                                                        "many_floats_optional",
                                                                        pa.list_(
                                                                            pa.field("item", pa.float32(), True, {})
                                                                        ),
                                                                        True,
                                                                        {},
                                                                    ),
                                                                    pa.field(
                                                                        "many_strings_required",
                                                                        pa.list_(
                                                                            pa.field("item", pa.utf8(), False, {})
                                                                        ),
                                                                        False,
                                                                        {},
                                                                    ),
                                                                    pa.field(
                                                                        "many_strings_optional",
                                                                        pa.list_(pa.field("item", pa.utf8(), True, {})),
                                                                        True,
                                                                        {},
                                                                    ),
                                                                    pa.field(
                                                                        "flattened_scalar", pa.float32(), False, {}
                                                                    ),
                                                                    pa.field(
                                                                        "almost_flattened_scalar",
                                                                        pa.struct(
                                                                            [pa.field("value", pa.float32(), False, {})]
                                                                        ),
                                                                        False,
                                                                        {},
                                                                    ),
                                                                ]
                                                            ),
                                                            False,
                                                            {},
                                                        )
                                                    ),
                                                    False,
                                                    {},
                                                ),
                                            ]
                                        ),
                                        False,
                                        {},
                                    )
                                ),
                                False,
                                {},
                            ),
                            pa.field(
                                "many_optional",
                                pa.list_(
                                    pa.field(
                                        "item",
                                        pa.dense_union(
                                            [
                                                pa.field("degrees", pa.float32(), False, {}),
                                                pa.field("radians", pa.float32(), False, {}),
                                                pa.field(
                                                    "craziness",
                                                    pa.list_(
                                                        pa.field(
                                                            "item",
                                                            pa.struct(
                                                                [
                                                                    pa.field(
                                                                        "single_float_optional", pa.float32(), True, {}
                                                                    ),
                                                                    pa.field(
                                                                        "single_string_required", pa.utf8(), False, {}
                                                                    ),
                                                                    pa.field(
                                                                        "single_string_optional", pa.utf8(), True, {}
                                                                    ),
                                                                    pa.field(
                                                                        "many_floats_optional",
                                                                        pa.list_(
                                                                            pa.field("item", pa.float32(), True, {})
                                                                        ),
                                                                        True,
                                                                        {},
                                                                    ),
                                                                    pa.field(
                                                                        "many_strings_required",
                                                                        pa.list_(
                                                                            pa.field("item", pa.utf8(), False, {})
                                                                        ),
                                                                        False,
                                                                        {},
                                                                    ),
                                                                    pa.field(
                                                                        "many_strings_optional",
                                                                        pa.list_(pa.field("item", pa.utf8(), True, {})),
                                                                        True,
                                                                        {},
                                                                    ),
                                                                    pa.field(
                                                                        "flattened_scalar", pa.float32(), False, {}
                                                                    ),
                                                                    pa.field(
                                                                        "almost_flattened_scalar",
                                                                        pa.struct(
                                                                            [pa.field("value", pa.float32(), False, {})]
                                                                        ),
                                                                        False,
                                                                        {},
                                                                    ),
                                                                ]
                                                            ),
                                                            False,
                                                            {},
                                                        )
                                                    ),
                                                    False,
                                                    {},
                                                ),
                                            ]
                                        ),
                                        True,
                                        {},
                                    )
                                ),
                                False,
                                {},
                            ),
                        ]
                    ),
                    True,
                    {},
                )
            ),
            "rerun.testing.components.AffixFuzzer18",
        )


class AffixFuzzer18Array(BaseExtensionArray[AffixFuzzer18ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer18"
    _EXTENSION_TYPE = AffixFuzzer18Type

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer18ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


AffixFuzzer18Type._ARRAY_TYPE = AffixFuzzer18Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer18Type())
