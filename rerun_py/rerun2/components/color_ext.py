from __future__ import annotations

from typing import Any, Optional, Sequence

import numpy as np
import pyarrow as pa


class ColorArrayExt:
    @staticmethod
    def from_similar(
        data: Optional[Any], *, mono: type, mono_aliases: type, many: type, many_aliases: type, arrow: type
    ):
        """
        Normalize flexible colors arrays.

        Float colors are assumed to be in 0-1 gamma sRGB space.
        All other colors are assumed to be in 0-255 gamma sRGB space.

        If there is an alpha, we assume it is in linear space, and separate (NOT pre-multiplied).
        """
        if isinstance(data, Sequence) and (len(data) > 0 and isinstance(data[0], mono)):
            # array = np.concatenate([np.asarray(radius) for radius in data], dtype=np.uint8)
            array = np.asarray([color.rgba for color in data], np.uint32).flatten()
        else:
            array = np.asarray(data).flatten()
            # TODO: all of this is very weird, color is supposed to be a u32 on the wire?!
            # Rust expects colors in 0-255 uint8
            if array.dtype.type in [np.float32, np.float64]:
                # Assume gamma-space colors
                array = np.require(np.round(array * 255.0), np.uint8)

        return arrow().wrap_array(pa.array(array, type=arrow().storage_type))
