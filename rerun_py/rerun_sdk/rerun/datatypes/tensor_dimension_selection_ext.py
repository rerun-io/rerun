from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, cast

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from .tensor_dimension_selection import TensorDimensionSelectionArrayLike


class TensorDimensionSelectionExt:
    """Extension for [TensorDimensionSelection][rerun.datatypes.TensorDimensionSelection]."""

    # TODO(#2641): this is needed until we support default values.
    def __init__(self: Any, dimension: int, *, invert: bool = False) -> None:
        """
        Create a new instance of the TensorDimensionSelection datatype.

        Parameters
        ----------
        dimension:
            The dimension number to select.
        invert:
            Invert the direction of the dimension.

        """

        # You can define your own __init__ function as a member of TensorDimensionSelectionExt in tensor_dimension_selection_ext.py
        self.__attrs_init__(dimension=dimension, invert=invert)

    @staticmethod
    def native_to_pa_array_override(data: TensorDimensionSelectionArrayLike, data_type: pa.DataType) -> pa.Array:
        from .tensor_dimension_selection import TensorDimensionSelection

        if isinstance(data, TensorDimensionSelection):
            data = [data]
        elif isinstance(data, np.ndarray):
            data = [TensorDimensionSelection(dimension=x) for x in range(data)]
        elif isinstance(data, int):
            data = [TensorDimensionSelection(dimension=data)]
        data = cast(Sequence[TensorDimensionSelection], data)

        return pa.StructArray.from_arrays(
            [
                pa.array(np.asarray([x.dimension for x in data], dtype=np.uint32)),
                pa.array(np.asarray([x.invert for x in data], dtype=np.bool_)),
            ],
            fields=list(data_type),
        )
