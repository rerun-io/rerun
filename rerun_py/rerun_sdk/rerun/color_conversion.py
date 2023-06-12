"""Color conversion utilities."""
from __future__ import annotations

import numpy as np
import numpy.typing as npt


def u8_array_to_rgba(arr: npt.NDArray[np.uint8]) -> npt.NDArray[np.uint32]:
    """
    Convert an array with inner dimension [R,G,B,A] into packed uint32 values.

    Parameters
    ----------
    arr :
        Nx3 or Nx4 `[[r,g,b,a], ... ]` of uint8 values

    Returns
    -------
    npt.NDArray[np.uint32]
        Array of uint32 value as 0xRRGGBBAA.

    """
    r = arr[:, 0]
    g = arr[:, 1]
    b = arr[:, 2]
    a = arr[:, 3] if arr.shape[1] == 4 else np.repeat(0xFF, len(arr))
    # Reverse the byte order because this is how we encode into uint32
    arr = np.vstack([a, b, g, r]).T
    # Make contiguous and then reinterpret
    arr = np.ascontiguousarray(arr, dtype=np.uint8)
    arr = arr.view(np.uint32)
    arr = np.squeeze(arr, axis=1)
    return arr  # type: ignore[return-value]


def linear_to_gamma_u8_value(linear: npt.NDArray[np.float32 | np.float64]) -> npt.NDArray[np.uint8]:
    """
    Transform color values from linear [0.0, 1.0] to gamma encoded [0, 255].

    Linear colors are expected to have dtype [numpy.floating][]

    Intended to implement the following per color value:
    ```Rust
    if l <= 0.0 {
        0
    } else if l <= 0.0031308 {
        round(3294.6 * l)
    } else if l <= 1.0 {
        round(269.025 * l.powf(1.0 / 2.4) - 14.025)
    } else {
        255
    }
    ```

    Parameters
    ----------
    linear:
        The linear color values to transform.

    Returns
    -------
    np.ndarray[np.uint8]
        The gamma encoded color values.

    """
    gamma = linear.clip(min=0, max=1)
    below = gamma <= 0.0031308
    gamma[below] *= 3294.6
    above = np.logical_not(below)
    gamma[above] = gamma[above] ** (1.0 / 2.4) * 269.025 - 14.025

    gamma.round(decimals=0, out=gamma)
    return gamma.astype(np.uint8)


def linear_to_gamma_u8_pixel(linear: npt.NDArray[np.float32 | np.float64]) -> npt.NDArray[np.uint8]:
    """
    Transform color pixels from linear [0, 1] to gamma encoded [0, 255].

    Linear colors are expected to have dtype np.float32 or np.float64.

    The last dimension of the colors array `linear` is expected to represent a single pixel color.
    - 3 colors means RGB
    - 4 colors means RGBA

    Parameters
    ----------
    linear:
        The linear color pixels to transform.

    Returns
    -------
    np.ndarray[np.uint8]
        The gamma encoded color pixels.

    """
    num_channels = linear.shape[-1]
    assert num_channels in (3, 4)

    if num_channels == 3:
        return linear_to_gamma_u8_value(linear)

    gamma_u8 = np.empty(shape=linear.shape, dtype=np.uint8)
    gamma_u8[..., :-1] = linear_to_gamma_u8_value(linear[..., :-1])
    gamma_u8[..., -1] = np.around(255 * linear[..., -1])

    return gamma_u8
