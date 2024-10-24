from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import numpy as np
import pyarrow as pa

from .._validators import flat_np_float64_array_from_array_like

if TYPE_CHECKING:
    from . import DVec2DArrayLike

NUMPY_VERSION = tuple(map(int, np.version.version.split(".")[:2]))


class DVec2DExt:
    """Extension for [DVec2D][rerun.datatypes.DVec2D]."""

    @staticmethod
    def native_to_pa_array_override(data: DVec2DArrayLike, data_type: pa.DataType) -> pa.Array:
        # TODO(ab): get rid of this once we drop support for Python 3.8. Make sure to pin numpy>=1.25.
        if NUMPY_VERSION < (1, 25):
            # Older numpy doesn't seem to support `data` in the form of [Point3D(1, 2), Point3D(3, 4)]
            # this happens for python 3.8 (1.25 supports 3.9+)
            from . import DVec2D

            if isinstance(data, Sequence):
                data = [np.array(p.xy) if isinstance(p, DVec2D) else p for p in data]

        points = flat_np_float64_array_from_array_like(data, 2)
        return pa.FixedSizeListArray.from_arrays(points, type=data_type)
