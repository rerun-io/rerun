# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/utf8_list.fbs".

# You can extend this class by creating a "Utf8ListExt" class in "utf8list_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, Union

import pyarrow as pa
from attrs import define, field

from ..._baseclasses import (
    BaseBatch,
)
from .utf8list_ext import Utf8ListExt

__all__ = ["Utf8List", "Utf8ListArrayLike", "Utf8ListBatch", "Utf8ListLike"]


@define(init=False)
class Utf8List(Utf8ListExt):
    """**Datatype**: A list of strings of text, encoded as UTF-8."""

    def __init__(self: Any, value: Utf8ListLike):
        """Create a new instance of the Utf8List datatype."""

        # You can define your own __init__ function as a member of Utf8ListExt in utf8list_ext.py
        self.__attrs_init__(value=value)

    value: list[str] = field(
        converter=Utf8ListExt.value__field_converter_override,  # type: ignore[misc]
    )


if TYPE_CHECKING:
    Utf8ListLike = Union[Utf8List, Sequence[str]]
else:
    Utf8ListLike = Any

Utf8ListArrayLike = Union[
    Utf8List,
    Sequence[Utf8ListLike],
]


class Utf8ListBatch(BaseBatch[Utf8ListArrayLike]):
    _ARROW_DATATYPE = pa.list_(pa.field("item", pa.utf8(), nullable=False, metadata={}))

    @staticmethod
    def _native_to_pa_array(data: Utf8ListArrayLike, data_type: pa.DataType) -> pa.Array:
        return Utf8ListExt.native_to_pa_array_override(data, data_type)
