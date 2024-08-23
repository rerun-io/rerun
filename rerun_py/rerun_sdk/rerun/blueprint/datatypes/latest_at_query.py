# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/latest_at_query.fbs".

# You can extend this class by creating a "LatestAtQueryExt" class in "latest_at_query_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    BaseBatch,
    BaseExtensionType,
)
from .latest_at_query_ext import LatestAtQueryExt

__all__ = ["LatestAtQuery", "LatestAtQueryArrayLike", "LatestAtQueryBatch", "LatestAtQueryLike", "LatestAtQueryType"]


def _latest_at_query__timeline__special_field_converter_override(x: datatypes.Utf8Like) -> datatypes.Utf8:
    if isinstance(x, datatypes.Utf8):
        return x
    else:
        return datatypes.Utf8(x)


@define(init=False)
class LatestAtQuery(LatestAtQueryExt):
    """**Datatype**: Latest-at query configuration for a specific timeline."""

    def __init__(self: Any, timeline: datatypes.Utf8Like, time: datatypes.TimeIntLike):
        """
        Create a new instance of the LatestAtQuery datatype.

        Parameters
        ----------
        timeline:
            Name of the timeline this applies to.
        time:
            Time value to use for this query.

        """

        # You can define your own __init__ function as a member of LatestAtQueryExt in latest_at_query_ext.py
        self.__attrs_init__(timeline=timeline, time=time)

    timeline: datatypes.Utf8 = field(converter=_latest_at_query__timeline__special_field_converter_override)
    # Name of the timeline this applies to.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    time: datatypes.TimeInt = field(
        converter=LatestAtQueryExt.time__field_converter_override,  # type: ignore[misc]
    )
    # Time value to use for this query.
    #
    # (Docstring intentionally commented out to hide this field from the docs)


LatestAtQueryLike = LatestAtQuery
LatestAtQueryArrayLike = Union[
    LatestAtQuery,
    Sequence[LatestAtQueryLike],
]


class LatestAtQueryType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.datatypes.LatestAtQuery"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct([
                pa.field("timeline", pa.utf8(), nullable=False, metadata={}),
                pa.field("time", pa.int64(), nullable=False, metadata={}),
            ]),
            self._TYPE_NAME,
        )


class LatestAtQueryBatch(BaseBatch[LatestAtQueryArrayLike]):
    _ARROW_TYPE = LatestAtQueryType()

    @staticmethod
    def _native_to_pa_array(data: LatestAtQueryArrayLike, data_type: pa.DataType) -> pa.Array:
        from rerun.datatypes import TimeIntBatch, Utf8Batch

        if isinstance(data, LatestAtQuery):
            data = [data]

        return pa.StructArray.from_arrays(
            [
                Utf8Batch([x.timeline for x in data]).as_arrow_array().storage,  # type: ignore[misc, arg-type]
                TimeIntBatch([x.time for x in data]).as_arrow_array().storage,  # type: ignore[misc, arg-type]
            ],
            fields=list(data_type),
        )
