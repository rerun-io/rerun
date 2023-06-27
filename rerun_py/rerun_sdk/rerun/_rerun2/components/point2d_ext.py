from __future__ import annotations

__all__ = ["Point2DArrayExt"]

from typing import Any, Sequence

import numpy as np
import pyarrow as pa


class Point2DArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        if isinstance(data, Sequence) and (len(data) > 0 and isinstance(data[0], mono)):
            points = np.concatenate([np.asarray(datum.position, dtype=np.float32) for datum in data])
        else:
            points = np.asarray(data, dtype=np.float32)

        points = points.reshape((-1,))

        return arrow().wrap_array(pa.FixedSizeListArray.from_arrays(points, type=arrow().storage_type))
