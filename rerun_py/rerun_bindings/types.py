from __future__ import annotations

from typing import TYPE_CHECKING, TypeAlias

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
