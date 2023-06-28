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
            points = np.concatenate([np.asarray([datum.x, datum.y], dtype=np.float32) for datum in data])
        else:
            points = np.asarray(data, dtype=np.float32)

        points = points.reshape((-1, 2))

        return arrow().wrap_array(
            pa.StructArray.from_arrays(
                arrays=[pa.array(c, type=pa.float32()) for c in points.T],
                fields=list(arrow().storage_type),
            )
        )
