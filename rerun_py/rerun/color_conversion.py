"""Color conversion utilities."""
from typing import Sequence, Union

import numpy as np
import numpy.typing as npt


def u8_array_to_rgba(arr: Sequence[int]) -> np.uint32:
    """
    Convert an array[4] of uint8 values into a uint32.

    Parameters
    ----------
    arr : Sequence[int]
        The array of uint8 values to convert in RGBA order.

    Returns
    -------
    int
        The uint32 value as 0xRRGGBBAA.

    """
    red = arr[0]
    green = arr[1]
    blue = arr[2]
    alpha = arr[3] if len(arr) == 4 else 0xFF
    return np.uint32((red << 24) + (green << 16) + (blue << 8) + alpha)


def linear_to_gamma_u8_value(linear: npt.NDArray[Union[np.float32, np.float64]]) -> npt.NDArray[np.uint8]:
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


def linear_to_gamma_u8_pixel(linear: npt.NDArray[Union[np.float32, np.float64]]) -> npt.NDArray[np.uint8]:
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
