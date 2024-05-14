# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/datatypes/utf8.fbs".

# You can extend this class by creating a "Utf8Ext" class in "utf8_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType

__all__ = ["Utf8", "Utf8ArrayLike", "Utf8Batch", "Utf8Like", "Utf8Type"]


@define(init=False)
class Utf8:
    """**Datatype**: A string of text, encoded as UTF-8."""

    def __init__(self: Any, value: Utf8Like):
        """Create a new instance of the Utf8 datatype."""

        # You can define your own __init__ function as a member of Utf8Ext in utf8_ext.py
        self.__attrs_init__(value=value)

    value: str = field(converter=str)

    def __str__(self) -> str:
        return str(self.value)

    def __hash__(self) -> int:
        return hash(self.value)


if TYPE_CHECKING:
    Utf8Like = Union[Utf8, str]
else:
    Utf8Like = Any

Utf8ArrayLike = Union[Utf8, Sequence[Utf8Like], str, Sequence[str]]


class Utf8Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.datatypes.Utf8"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), self._TYPE_NAME)


class Utf8Batch(BaseBatch[Utf8ArrayLike]):
    _ARROW_TYPE = Utf8Type()

    @staticmethod
    def _native_to_pa_array(data: Utf8ArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, str):
            array = [data]
        elif isinstance(data, Sequence):
            array = [str(datum) for datum in data]
        else:
            array = [str(data)]

        return pa.array(array, type=data_type)
