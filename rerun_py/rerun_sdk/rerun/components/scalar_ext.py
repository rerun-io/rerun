from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import ScalarArrayLike


class ScalarExt:
    """Extension for [Scalar][rerun.components.Scalar]."""

    @staticmethod
    def native_to_pa_array_override(data: ScalarArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.float64).flatten()
        return pa.array(array, type=data_type)
