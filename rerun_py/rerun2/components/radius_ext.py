from __future__ import annotations

from typing import Any, Optional, Sequence

import numpy as np
import pyarrow as pa


class RadiusArrayExt:
    @staticmethod
    def from_similar(
        data: Optional[Any], *, mono: type, mono_aliases: type, many: type, many_aliases: type, arrow: type
    ):
        if isinstance(data, Sequence) and (len(data) > 0 and isinstance(data[0], mono)):
            array = np.asarray([radius.value for radius in data], np.float32)
        else:
            array = np.require(np.asarray(data), np.float32).flatten()

        return arrow().wrap_array(pa.array(array, type=arrow().storage_type))
