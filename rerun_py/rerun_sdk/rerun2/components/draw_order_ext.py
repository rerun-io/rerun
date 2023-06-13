from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import pyarrow as pa


class DrawOrderArrayExt:
    @staticmethod
    def _from_similar(data: Any | None, *, mono: type, mono_aliases: type, many: type, many_aliases: type, arrow: type):
        if isinstance(data, Sequence) and (len(data) > 0 and isinstance(data[0], mono)):
            array = np.asarray([draw_order.value for draw_order in data], np.float32)
        else:
            array = np.require(np.asarray(data), np.float32).flatten()

        return arrow().wrap_array(pa.array(array, type=arrow().storage_type))
