# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/aggregation_policy.fbs".

# You can extend this class by creating a "AggregationPolicyExt" class in "aggregation_policy_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = [
    "AggregationPolicy",
    "AggregationPolicyArrayLike",
    "AggregationPolicyBatch",
    "AggregationPolicyLike",
    "AggregationPolicyType",
]


from enum import Enum


class AggregationPolicy(Enum):
    """
    **Component**: Policy for aggregation of multiple scalar plot values.

    This is used for lines in plots when the X axis distance of individual points goes below a single pixel,
    i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
    (and readability) in such situations as it prevents overdraw.
    """

    Off = 1
    """No aggregation."""

    Average = 2
    """Average all points in the range together."""

    Max = 3
    """Keep only the maximum values in the range."""

    Min = 4
    """Keep only the minimum values in the range."""

    MinMax = 5
    """
    Keep both the minimum and maximum values in the range.

    This will yield two aggregated points instead of one, effectively creating a vertical line.
    """

    MinMaxAverage = 6
    """Find both the minimum and maximum values in the range, then use the average of those."""


AggregationPolicyLike = Union[
    AggregationPolicy,
    Literal["off"]
    | Literal["average"]
    | Literal["max"]
    | Literal["min"]
    | Literal["minmax"]
    | Literal["minmaxaverage"],
]
AggregationPolicyArrayLike = Union[AggregationPolicyLike, Sequence[AggregationPolicyLike]]


class AggregationPolicyType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.AggregationPolicy"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.sparse_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field("Off", pa.null(), nullable=True, metadata={}),
                pa.field("Average", pa.null(), nullable=True, metadata={}),
                pa.field("Max", pa.null(), nullable=True, metadata={}),
                pa.field("Min", pa.null(), nullable=True, metadata={}),
                pa.field("MinMax", pa.null(), nullable=True, metadata={}),
                pa.field("MinMaxAverage", pa.null(), nullable=True, metadata={}),
            ]),
            self._TYPE_NAME,
        )


class AggregationPolicyBatch(BaseBatch[AggregationPolicyArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = AggregationPolicyType()

    @staticmethod
    def _native_to_pa_array(data: AggregationPolicyArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (AggregationPolicy, int, str)):
            data = [data]

        types: list[int] = []

        for value in data:
            if value is None:
                types.append(0)
            elif isinstance(value, AggregationPolicy):
                types.append(value.value)  # Actual enum value
            elif isinstance(value, int):
                types.append(value)  # By number
            elif isinstance(value, str):
                if hasattr(AggregationPolicy, value):
                    types.append(AggregationPolicy[value].value)  # fast path
                elif value.lower() == "off":
                    types.append(AggregationPolicy.Off.value)
                elif value.lower() == "average":
                    types.append(AggregationPolicy.Average.value)
                elif value.lower() == "max":
                    types.append(AggregationPolicy.Max.value)
                elif value.lower() == "min":
                    types.append(AggregationPolicy.Min.value)
                elif value.lower() == "minmax":
                    types.append(AggregationPolicy.MinMax.value)
                elif value.lower() == "minmaxaverage":
                    types.append(AggregationPolicy.MinMaxAverage.value)
                else:
                    raise ValueError(f"Unknown AggregationPolicy kind: {value}")
            else:
                raise ValueError(f"Unknown AggregationPolicy kind: {value}")

        buffers = [
            None,
            pa.array(types, type=pa.int8()).buffers()[1],
        ]
        children = (1 + 6) * [pa.nulls(len(data))]

        return pa.UnionArray.from_buffers(
            type=data_type,
            length=len(data),
            buffers=buffers,
            children=children,
        )
