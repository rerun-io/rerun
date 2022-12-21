from typing import Optional

import numpy as np
import numpy.typing as npt
from rerun.log import Colors
from rerun.log.tensor import _log_tensor, log_tensor

from rerun import rerun_bindings  # type: ignore[attr-defined]

__all__ = [
    "log_image",
    "log_depth_image",
    "log_segmentation_image",
]


def log_image(
    obj_path: str,
    image: Colors,
    *,
    timeless: bool = False,
) -> None:
    """
    Log a gray or color image.

    The image should either have 1, 3 or 4 channels (gray, RGB or RGBA).

    Supported `dtype`s:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
    * uint16: color components should be in 0-65535 sRGB gamma space, except for alpha which should be in 0-65535
    linear space.
    * float32/float64: all color components should be in 0-1 linear space.

    """
    # Catch some errors early:
    if len(image.shape) < 2 or 3 < len(image.shape):
        raise TypeError(f"Expected image, got array of shape {image.shape}")

    if len(image.shape) == 3:
        depth = image.shape[2]
        if depth not in (1, 3, 4):
            raise TypeError(
                f"Expected image depth of 1 (gray), 3 (RGB) or 4 (RGBA). Instead got array of shape {image.shape}"
            )

    log_tensor(obj_path, image, timeless=timeless)


def log_depth_image(
    obj_path: str,
    image: Colors,
    *,
    meter: Optional[float] = None,
    timeless: bool = False,
) -> None:
    """
    Log a depth image.

    The image must be a 2D array. Supported `dtype`:s are: uint8, uint16, float32, float64

    meter: How long is a meter in the given dtype?
           For instance: with uint16, perhaps meter=1000 which would mean
           you have millimeter precision and a range of up to ~65 meters (2^16 / 1000).

    """
    # Catch some errors early:
    if len(image.shape) != 2:
        raise TypeError(f"Expected 2D depth image, got array of shape {image.shape}")

    log_tensor(obj_path, image, meter=meter, timeless=timeless)


def log_segmentation_image(
    obj_path: str,
    image: npt.ArrayLike,
    *,
    timeless: bool = False,
) -> None:
    """
    Log an image made up of integer class-ids.

    The image should have 1 channel, i.e. be either `H x W` or `H x W x 1`.
    """
    if not isinstance(image, np.ndarray):
        image = np.array(image, dtype=np.uint16)

    # Catch some errors early:
    if len(image.shape) < 2 or 3 < len(image.shape):
        raise TypeError(f"Expected image, got array of shape {image.shape}")

    if len(image.shape) == 3:
        depth = image.shape[2]
        if depth != 1:
            raise TypeError(f"Expected image depth of 1. Instead got array of shape {image.shape}")

    _log_tensor(obj_path, tensor=image, meaning=rerun_bindings.TensorDataMeaning.ClassId, timeless=timeless)
