from __future__ import annotations

from typing import Any, Optional, Sequence

import pyarrow as pa


class LabelArrayExt:
    @staticmethod
    def from_similar(
        data: Optional[Any], *, mono: type, mono_aliases: type, many: type, many_aliases: type, arrow: type
    ):
        if isinstance(data, Sequence):
            array = [str(datum) for datum in data]
        else:
            array = [str(data)]

        return arrow().wrap_array(pa.array(array, type=arrow().storage_type))
