from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from . import VisibleArrayLike


class VisibleExt:
    """Extension for [Visible][rerun.blueprint.components.Visible]."""

    @staticmethod
    def native_to_pa_array_override(data: VisibleArrayLike, data_type: pa.DataType) -> pa.Array:
        array = np.asarray(data, dtype=np.bool_).flatten()
        return pa.array(array, type=data_type)
