from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.color_conversion import u8_array_to_rgba

if TYPE_CHECKING:
    from . import ColorArrayLike, ColorLike


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


class ColorExt:
    """
    Extension for the `Color` datatype.

    Possible input for `Color`:
    - Sequence[int]: interpreted as rgb or rgba values in 0-255 range
    - numpy array: interpreted as rgb or rgba values, range depending on dtype
    - anything else (int or convertible to int): interpreted as a 32-bit packed rgba value

    Possible inputs for `ColorBatch()`:
    - a single `Color` instance
    - a sequence of `Color` instances
    - Nx3 or Nx4 numpy array, range depending on dtype
    """

    @staticmethod
    def rgba__field_converter_override(data: ColorLike) -> int:
        from . import Color

        if isinstance(data, Color):
            return data.rgba
        if isinstance(data, np.ndarray):
            return int(_numpy_array_to_u32(data.reshape((1, -1)))[0])
        elif isinstance(data, Sequence):
            data = np.array(data).reshape((1, -1))
            if data.shape[1] not in (3, 4):
                raise ValueError(f"expected sequence of length of 3 or 4, received {data.shape[1]}")
            return int(_numpy_array_to_u32(data)[0])
        else:
            return int(data)

    @staticmethod
    def native_to_pa_array_override(data: ColorArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import Color

        if isinstance(data, int) or isinstance(data, Color):
            # A single packed int or Color (which implements __int__())
            int_array = np.array([data])
        elif isinstance(data, Sequence) and len(data) == 0:
            # An empty array
            int_array = np.array([])
        else:
            # Try to coerce it to a numpy array
            try:
                arr = np.asarray(data)

                if arr.dtype == np.uint32:
                    # these are already packed values
                    int_array = arr.flatten()
                else:
                    # these are component values
                    if len(arr.shape) == 1:
                        if arr.size > 4:
                            # multiple RGBA colors
                            arr = arr.reshape((-1, 4))
                        else:
                            # a single color
                            arr = arr.reshape((1, -1))
                    int_array = _numpy_array_to_u32(arr)
            except (ValueError, TypeError, IndexError):
                # Fallback support
                data_list = list(data)  # type: ignore[arg-type]

                # First try to coerce it to a single Color instance
                try:
                    data_list = [Color(data_list)]  # type: ignore[arg-type]
                except (IndexError, ValueError):
                    pass

                # Fially, handle heterogeneous sequence of Color-like object,
                # such as Color instances, ints, sub-sequence, etc.
                #
                # Note how this is simplified by the flexible implementation of
                # `Color`, thanks to its converter function and the
                # auto-generated `__int__()` method.
                int_array = np.array([Color(datum) for datum in data_list], np.uint32)  # type: ignore[arg-type]

        return pa.array(int_array, type=data_type)
