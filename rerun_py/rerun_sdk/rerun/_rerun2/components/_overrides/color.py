"""
Overrides for `Color` component.

Possible input for `Color`:
- Sequence[int]: interpreted as rgb or rgba values in 0-255 range
- numpy array: interpreted as rgb or rgba values, range depending on dtype
- anything else (int or convertible to int): interpreted as a 32-bit packed rgba value

Possible inputs for `ColorArray.from_similar()`:
- a single `Color` instance
- a sequence of `Color` instances
- Nx3 or Nx4 numpy array, range depending on dtype
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Sequence, Union, cast

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.color_conversion import u8_array_to_rgba

if TYPE_CHECKING:
    from .. import ColorArrayLike, ColorLike


def _numpy_array_to_u32(data: npt.NDArray[np.uint8 | np.float32 | np.float64]) -> npt.NDArray[np.uint32]:
    if data.size == 0:
        return np.array([], dtype=np.uint32)

    if data.dtype.type in [np.float32, np.float64]:
        array = u8_array_to_rgba(np.asarray(np.round(np.asarray(data) * 255.0), np.uint8))
    elif data.dtype.type == np.uint32:
        array = np.asarray(data, dtype=np.uint32).flatten()
    else:
        array = u8_array_to_rgba(np.asarray(data, dtype=np.uint8))
    return array


def color_rgba_converter(data: ColorLike) -> int:
    if isinstance(data, np.ndarray):
        return int(_numpy_array_to_u32(data.reshape((1, -1)))[0])
    elif isinstance(data, Sequence):
        return int(_numpy_array_to_u32(np.array(data, dtype=np.uint8).reshape((1, -1)))[0])
    else:
        return int(data)


def color_native_to_pa_array(data: ColorArrayLike, data_type: pa.DataType) -> pa.Array:
    from .. import Color

    if isinstance(data, Color):
        data = [data]

    if isinstance(data, np.ndarray):
        if data.dtype == np.uint32:
            # these are already packed values
            array = data.flatten()
        else:
            # these are component values
            if len(data.shape) == 1:
                if data.size > 4:
                    # multiple RGBA colors
                    data = data.reshape((-1, 4))
                else:
                    # a single color
                    data = data.reshape((1, -1))
            array = _numpy_array_to_u32(cast(npt.NDArray[Union[np.uint8, np.float32, np.float64]], data))
    else:
        # Sequence of Color (which are int-like) or sequence of sequence
        array = np.array([Color(datum) for datum in data], np.uint32)  # type: ignore[union-attr]

    return pa.array(array, type=data_type)
