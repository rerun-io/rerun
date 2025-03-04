# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/visible_time_range.fbs".

# You can extend this class by creating a "TimeRangeExt" class in "time_range_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseBatch,
)

__all__ = ["TimeRange", "TimeRangeArrayLike", "TimeRangeBatch", "TimeRangeLike"]


def _time_range__start__special_field_converter_override(
    x: datatypes.TimeRangeBoundaryLike,
) -> datatypes.TimeRangeBoundary:
    if isinstance(x, datatypes.TimeRangeBoundary):
        return x
    else:
        return datatypes.TimeRangeBoundary(x)


def _time_range__end__special_field_converter_override(
    x: datatypes.TimeRangeBoundaryLike,
) -> datatypes.TimeRangeBoundary:
    if isinstance(x, datatypes.TimeRangeBoundary):
        return x
    else:
        return datatypes.TimeRangeBoundary(x)


@define(init=False)
class TimeRange:
    """**Datatype**: Visible time range bounds for a specific timeline."""

    def __init__(self: Any, start: datatypes.TimeRangeBoundaryLike, end: datatypes.TimeRangeBoundaryLike):
        """
        Create a new instance of the TimeRange datatype.

        Parameters
        ----------
        start:
            Low time boundary for sequence timeline.
        end:
            High time boundary for sequence timeline.

        """

        # You can define your own __init__ function as a member of TimeRangeExt in time_range_ext.py
        self.__attrs_init__(start=start, end=end)

    start: datatypes.TimeRangeBoundary = field(converter=_time_range__start__special_field_converter_override)
    # Low time boundary for sequence timeline.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    end: datatypes.TimeRangeBoundary = field(converter=_time_range__end__special_field_converter_override)
    # High time boundary for sequence timeline.
    #
    # (Docstring intentionally commented out to hide this field from the docs)


TimeRangeLike = TimeRange
TimeRangeArrayLike = Union[
    TimeRange,
    Sequence[TimeRangeLike],
]


class TimeRangeBatch(BaseBatch[TimeRangeArrayLike]):
    _ARROW_DATATYPE = pa.struct([
        pa.field(
            "start",
            pa.dense_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field("CursorRelative", pa.int64(), nullable=False, metadata={}),
                pa.field("Absolute", pa.int64(), nullable=False, metadata={}),
                pa.field("Infinite", pa.null(), nullable=True, metadata={}),
            ]),
            nullable=True,
            metadata={},
        ),
        pa.field(
            "end",
            pa.dense_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field("CursorRelative", pa.int64(), nullable=False, metadata={}),
                pa.field("Absolute", pa.int64(), nullable=False, metadata={}),
                pa.field("Infinite", pa.null(), nullable=True, metadata={}),
            ]),
            nullable=True,
            metadata={},
        ),
    ])

    @staticmethod
    def _native_to_pa_array(data: TimeRangeArrayLike, data_type: pa.DataType) -> pa.Array:
        from rerun.datatypes import TimeRangeBoundaryBatch

        if isinstance(data, TimeRange):
            data = [data]

        return pa.StructArray.from_arrays(
            [
                TimeRangeBoundaryBatch([x.start for x in data]).as_arrow_array(),  # type: ignore[misc, arg-type]
                TimeRangeBoundaryBatch([x.end for x in data]).as_arrow_array(),  # type: ignore[misc, arg-type]
            ],
            fields=list(data_type),
        )
