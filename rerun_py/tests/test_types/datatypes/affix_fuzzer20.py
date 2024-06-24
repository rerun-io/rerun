# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer20Ext" class in "affix_fuzzer20_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import (
    BaseBatch,
    BaseExtensionType,
)

from .. import datatypes

__all__ = ["AffixFuzzer20", "AffixFuzzer20ArrayLike", "AffixFuzzer20Batch", "AffixFuzzer20Like", "AffixFuzzer20Type"]


def _affix_fuzzer20__p__special_field_converter_override(
    x: datatypes.PrimitiveComponentLike,
) -> datatypes.PrimitiveComponent:
    if isinstance(x, datatypes.PrimitiveComponent):
        return x
    else:
        return datatypes.PrimitiveComponent(x)


def _affix_fuzzer20__s__special_field_converter_override(x: datatypes.StringComponentLike) -> datatypes.StringComponent:
    if isinstance(x, datatypes.StringComponent):
        return x
    else:
        return datatypes.StringComponent(x)


@define(init=False)
class AffixFuzzer20:
    def __init__(self: Any, p: datatypes.PrimitiveComponentLike, s: datatypes.StringComponentLike):
        """Create a new instance of the AffixFuzzer20 datatype."""

        # You can define your own __init__ function as a member of AffixFuzzer20Ext in affix_fuzzer20_ext.py
        self.__attrs_init__(p=p, s=s)

    p: datatypes.PrimitiveComponent = field(converter=_affix_fuzzer20__p__special_field_converter_override)
    s: datatypes.StringComponent = field(converter=_affix_fuzzer20__s__special_field_converter_override)


AffixFuzzer20Like = AffixFuzzer20
AffixFuzzer20ArrayLike = Union[
    AffixFuzzer20,
    Sequence[AffixFuzzer20Like],
]


class AffixFuzzer20Type(BaseExtensionType):
    _TYPE_NAME: str = "rerun.testing.datatypes.AffixFuzzer20"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct([
                pa.field("p", pa.uint32(), nullable=False, metadata={}),
                pa.field("s", pa.utf8(), nullable=False, metadata={}),
            ]),
            self._TYPE_NAME,
        )


class AffixFuzzer20Batch(BaseBatch[AffixFuzzer20ArrayLike]):
    _ARROW_TYPE = AffixFuzzer20Type()

    @staticmethod
    def _native_to_pa_array(data: AffixFuzzer20ArrayLike, data_type: pa.DataType) -> pa.Array:
        from rerun.testing.datatypes import PrimitiveComponentBatch, StringComponentBatch

        if isinstance(data, AffixFuzzer20):
            data = [data]

        return pa.StructArray.from_arrays(
            [
                PrimitiveComponentBatch([x.p for x in data]).as_arrow_array().storage,  # type: ignore[misc, arg-type]
                StringComponentBatch([x.s for x in data]).as_arrow_array().storage,  # type: ignore[misc, arg-type]
            ],
            fields=list(data_type),
        )
