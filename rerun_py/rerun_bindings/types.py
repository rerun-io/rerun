from __future__ import annotations

from typing import TYPE_CHECKING, Sequence, TypeAlias, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa

if TYPE_CHECKING:
    from rerun._baseclasses import ComponentMixin

    from .rerun_bindings import (
        ComponentColumnDescriptor as ComponentColumnDescriptor,
        ComponentColumnSelector as ComponentColumnSelector,
        IndexColumnSelector as IndexColumnDescriptor,
        IndexColumnSelector as IndexColumnSelector,
    )

ComponentLike: TypeAlias = Union[str, type["ComponentMixin"]]

AnyColumn: TypeAlias = Union[
    "ComponentColumnDescriptor",
    "ComponentColumnSelector",
    "IndexColumnDescriptor",
    "IndexColumnSelector",
]

AnyComponentColumn: TypeAlias = Union[
    "ComponentColumnDescriptor",
    "ComponentColumnSelector",
]

ViewContentsLike: TypeAlias = Union[
    str,
    dict[str, Union[AnyColumn, Sequence[ComponentLike]]],
]

IndexValuesLike: TypeAlias = Union[npt.NDArray[np.int_], pa.Int64Array]
