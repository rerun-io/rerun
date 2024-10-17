from __future__ import annotations

from typing import TYPE_CHECKING, Dict, Sequence, Type, Union

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

ComponentLike: TypeAlias = Union[str, Type["ComponentMixin"]]
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
    Dict[str, Union[AnyColumn, Sequence[ComponentLike]]],
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
