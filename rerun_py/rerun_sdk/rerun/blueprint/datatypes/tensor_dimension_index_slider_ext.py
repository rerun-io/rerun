from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, cast

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from .tensor_dimension_index_slider import TensorDimensionIndexSliderArrayLike


class TensorDimensionIndexSliderExt:
    """Extension for [TensorDimensionIndexSlider][rerun.blueprint.datatypes.TensorDimensionIndexSlider]."""

    @staticmethod
    def native_to_pa_array_override(data: TensorDimensionIndexSliderArrayLike, data_type: pa.DataType) -> pa.Array:
        from .tensor_dimension_index_slider import TensorDimensionIndexSlider

        if isinstance(data, TensorDimensionIndexSlider):
            data = [data]
        data = cast(Sequence[TensorDimensionIndexSlider | int], data)

        return pa.StructArray.from_arrays(
            [
                pa.array(
                    np.asarray(
                        [x.dimension if isinstance(x, TensorDimensionIndexSlider) else x for x in data],
                        dtype=np.uint32,
                    ),
                ),
            ],
            fields=list(data_type),
        )
