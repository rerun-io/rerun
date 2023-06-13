from __future__ import annotations

__all__ = ["RadiusArrayExt"]

from typing import Any, Sequence, Type

import numpy as np
import pyarrow as pa


class RadiusArrayExt:
    @staticmethod
    def _from_similar(data: Any | None, *, mono: type, mono_aliases: Type, many: type, many_aliases: Type, arrow: type):
        if isinstance(data, Sequence) and (len(data) > 0 and isinstance(data[0], mono)):
            array = np.asarray([radius.value for radius in data], np.float32)
        else:
            array = np.require(np.asarray(data), np.float32).flatten()

        return arrow().wrap_array(pa.array(array, type=arrow().storage_type))
