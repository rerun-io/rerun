from __future__ import annotations

__all__ = ["LabelArrayExt"]

from typing import Any, Sequence, Type

import pyarrow as pa


class LabelArrayExt:
    @staticmethod
    def _from_similar(data: Any | None, *, mono: type, mono_aliases: Type, many: type, many_aliases: Type, arrow: type):
        if isinstance(data, Sequence):
            array = [str(datum) for datum in data]
        else:
            array = [str(data)]

        return arrow().wrap_array(pa.array(array, type=arrow().storage_type))
