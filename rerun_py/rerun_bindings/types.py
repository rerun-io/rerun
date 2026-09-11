from __future__ import annotations

from typing import TYPE_CHECKING, Literal, TypeAlias, TypedDict

import numpy as np
import numpy.typing as npt
import pyarrow as pa

if TYPE_CHECKING:
    from .rerun_bindings import (
        ComponentColumnDescriptor as ComponentColumnDescriptor,
        ComponentColumnSelector as ComponentColumnSelector,
        ComponentDescriptor as ComponentDescriptor,
        IndexColumnDescriptor as IndexColumnDescriptor,
        IndexColumnSelector as IndexColumnSelector,
    )

IndexValuesLike: TypeAlias = npt.NDArray[np.int_] | npt.NDArray[np.datetime64] | pa.Int64Array
"""
A type alias for index values.

This can be any numpy-compatible array of integers, or a [`pyarrow.Int64Array`][]
"""

TableLike: TypeAlias = pa.Table | pa.RecordBatch | pa.RecordBatchReader
"""
A type alias for TableLike pyarrow objects.
"""

TemporalTimelineType: TypeAlias = Literal["duration_ns", "timestamp_ns", "duration", "timestamp"]

TimelineType: TypeAlias = TemporalTimelineType | Literal["sequence"]


class MergeSplitSettingsDict(TypedDict):
    """Wire form of `rerun.experimental._MergeSplitSettings`; `0` disables a row guard."""

    max_bytes: int
    max_rows: int
    max_rows_if_unsorted: int


class OwnChunkRuleDict(TypedDict):
    """
    Wire form of `rerun.experimental._OwnChunkRule`.

    Exactly one of `component_type` and `component` is set. `entity_filter` holds entity path filter
    rules, newline-separated, or `None` for every entity.
    """

    component_type: str | None
    component: str | None
    entity_filter: str | None
    merge_split: Literal["inherit", "passthrough"] | MergeSplitSettingsDict
