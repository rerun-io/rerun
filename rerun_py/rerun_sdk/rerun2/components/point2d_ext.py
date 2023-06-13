from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import pyarrow as pa


class Point2DArrayExt:
    @staticmethod
    def _from_similar(data: Any | None, *, mono: type, mono_aliases: type, many: type, many_aliases: type, arrow: type):
        if isinstance(data, Sequence) and (len(data) > 0 and isinstance(data[0], mono)):
            arrays = [np.asarray(datum) for datum in data]
        else:
            arrays = np.require(np.asarray(data), np.float32).reshape((-1, 2)).tolist()

        return arrow().wrap_array(pa.array(arrays, type=arrow().storage_type))
