from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Literal, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from typing_extensions import TypeAlias

if TYPE_CHECKING:
    from rerun._baseclasses import ComponentMixin

    from .rerun_bindings import (
        ComponentColumnDescriptor as ComponentColumnDescriptor,
        ComponentColumnSelector as ComponentColumnSelector,
        IndexColumnDescriptor as IndexColumnDescriptor,
        IndexColumnSelector as IndexColumnSelector,
        VectorDistanceMetric as VectorDistanceMetric,
    )

AnyColumn: TypeAlias = Union[
    str,
    "ComponentColumnDescriptor",
    "ComponentColumnSelector",
    "IndexColumnDescriptor",
    "IndexColumnSelector",
]
"""A type alias for any column-like object."""


AnyComponentColumn: TypeAlias = Union[
    str,
    "ComponentColumnDescriptor",
    "ComponentColumnSelector",
]
"""A type alias for any component-column-like object."""

ComponentLike: TypeAlias = Union[str, type["ComponentMixin"]]
"""
A type alias for a component-like object used for content-expressions and column selectors.

This can be the name of the component as a string, or an instance of the component class itself.
Strings are not required to be fully-qualified. Rerun will find the best-matching component
based on the corresponding entity.

Examples:

- `"rerun.components.Position3D"`
- `"Position3D"`
- `rerun.components.Position3D`
"""

ViewContentsLike: TypeAlias = Union[
    str,
    dict[str, Union[AnyColumn, ComponentLike, Sequence[ComponentLike]]],
]
"""
A type alias for specifying the contents of a view.

This can be a single string content-expression such as: `"world/cameras/**"`, or a dictionary
specifying multiple content-expressions and a respective list of components to select within
that expression such as `{"world/cameras/**": ["ImageBuffer", "PinholeProjection"]}`.
"""

IndexValuesLike: TypeAlias = Union[npt.NDArray[np.int_], pa.Int64Array]
"""
A type alias for index values.

This can be any numpy-compatible array of integers, or a [`pa.Int64Array`][]
"""

TableLike: TypeAlias = Union[pa.Table, pa.RecordBatch, pa.RecordBatchReader]
"""
A type alias for TableLike pyarrow objects.
"""

VectorDistanceMetricLike: TypeAlias = Union["VectorDistanceMetric", Literal["L2", "Cosine", "Dot", "Hamming"]]
"""
A type alias for vector distance metrics.
"""

VectorLike = Union[npt.NDArray[np.float64], list[float]]
"""
A type alias for vector-like objects.
"""
