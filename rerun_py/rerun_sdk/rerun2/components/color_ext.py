from __future__ import annotations

__all__ = ["ColorArrayExt"]

from typing import Any, Sequence

import numpy as np
import pyarrow as pa

from rerun.color_conversion import u8_array_to_rgba


class ColorArrayExt:
    @staticmethod
    def _from_similar(
        data: Any | None, *, mono: type, mono_aliases: Any, many: type, many_aliases: Any, arrow: type
    ) -> pa.Array:
        """
        Normalize flexible colors arrays.

        Float colors are assumed to be in 0-1 gamma sRGB space.
        All other colors are assumed to be in 0-255 gamma sRGB space.

        If there is an alpha, we assume it is in linear space, and separate (NOT pre-multiplied).
        """
        if isinstance(data, Sequence) and len(data) == 0:
            array = np.array([], np.uint32)
        elif isinstance(data, Sequence) and (len(data) > 0 and isinstance(data[0], mono)):
            array = np.asarray([color.rgba for color in data], np.uint32)
        elif isinstance(data, Sequence) and (len(data) > 0 and isinstance(data[0], int)):
            array = np.asarray(data, np.uint32)
        else:
            array = np.asarray(data)
            # Rust expects colors in 0-255 uint8
            if array.dtype.type in [np.float32, np.float64]:
                # Assume gamma-space colors
                array = np.asarray(data).reshape((-1, 4))
                array = u8_array_to_rgba(np.asarray(np.round(array * 255.0), np.uint8))
            elif array.dtype.type == np.uint32:
                array = np.asarray(data).flatten()
            else:
                array = np.asarray(data).reshape((-1, 4))
                array = u8_array_to_rgba(array)

        return arrow().wrap_array(pa.array(array, type=arrow().storage_type))
