from __future__ import annotations

__all__ = ["InstanceKeyArrayExt"]

from typing import Any, Sequence

import numpy as np
import pyarrow as pa


class InstanceKeyArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        if isinstance(data, Sequence) and (len(data) > 0 and isinstance(data[0], mono)):
            array = np.asarray([datum.value for datum in data], np.uint64)
        else:
            array = np.asarray(data, dtype=np.uint64).flatten()

        return arrow().wrap_array(pa.array(array, type=arrow().storage_type))
