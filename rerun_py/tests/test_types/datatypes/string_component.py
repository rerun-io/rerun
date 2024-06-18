# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy_deps.fbs".

# You can extend this class by creating a "StringComponentExt" class in "string_component_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import (
    BaseBatch,
    BaseExtensionType,
)

__all__ = [
    "StringComponent",
    "StringComponentArrayLike",
    "StringComponentBatch",
    "StringComponentLike",
    "StringComponentType",
]


@define(init=False)
class StringComponent:
    def __init__(self: Any, value: StringComponentLike):
        """Create a new instance of the StringComponent datatype."""

        # You can define your own __init__ function as a member of StringComponentExt in string_component_ext.py
        self.__attrs_init__(value=value)

    value: str = field(converter=str)

    def __str__(self) -> str:
        return str(self.value)

    def __hash__(self) -> int:
        return hash(self.value)


StringComponentLike = StringComponent
StringComponentArrayLike = Union[
    StringComponent,
    Sequence[StringComponentLike],
]


class StringComponentType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.datatypes.StringComponent"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), self._TYPE_NAME)


class StringComponentBatch(BaseBatch[StringComponentArrayLike]):
    _ARROW_TYPE = StringComponentType()

    @staticmethod
    def _native_to_pa_array(data: StringComponentArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, str):
            array = [data]
        elif isinstance(data, Sequence):
            array = [str(datum) for datum in data]
        else:
            array = [str(data)]

        return pa.array(array, type=data_type)
