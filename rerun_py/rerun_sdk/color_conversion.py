"""Color conversion utilities."""
from typing import Any, Union

import numpy as np

def linear_to_gamma_u8_value(linear: np.ndarray[Any, Any]) -> np.ndarray[Any, np.dtype[np.uint8]]:
    """Transform color values from linear [0, 1] to gamma encoded [0, 255].
    Linear colors are expected to have dtype np.float32 or np.float64.

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
    """
    gamma = linear.clip(min=0, max=1)
    below = gamma <= 0.0031308
    gamma[below] *= 3294.6
    above = np.logical_not(below)
    gamma[above] = gamma[above]**(1.0 / 2.4) * 269.025 - 14.025

    gamma.round(decimals=0, out=gamma)
    return gamma.astype(np.uint8)


def linear_to_gamma_u8_pixel(linear: np.ndarray[Any, Any]) -> np.ndarray[Any, np.dtype[np.uint8]]:
    """Transform color pixels from linear [0, 1] to gamma encoded [0, 255].

    Linear colors are expected to have dtype np.float32 or np.float64.

    The last dimension of the colors array `linear` is expected to represent a single pixel color.
    - 3 colors means RGB
    - 4 colors means RGBA

    """
    num_channels = linear.shape[-1]
    assert num_channels in (3, 4)

    if num_channels == 3:
        return linear_to_gamma_u8_value(linear)

    gamma_u8 = np.empty(shape=linear.shape, dtype=np.uint8)
    gamma_u8[..., :-1] = linear_to_gamma_u8_value(linear[..., :-1])
    gamma_u8[..., -1] = np.around(255 * linear[..., -1])

    return gamma_u8
