import logging
from typing import Any, Optional, Tuple

import numpy as np
import numpy.typing as npt
from rerun.log import Colors
from rerun.log.error_utils import _send_warning
from rerun.log.tensor import _log_tensor

from rerun import bindings

__all__ = [
    "log_image",
    "log_depth_image",
    "log_segmentation_image",
]


def _get_image_shape(image: Any) -> Tuple[int, ...]:
    try:
        return image.shape  # type: ignore[no-any-return]
    except AttributeError:
        size = image.size  # If it's a Pillow image, this will be a (width, height) tuple
        if isinstance(size, tuple):
            return size
        return (len(image),)


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
    shape = _get_image_shape(image)
    non_empty_dims = [d for d in shape if d != 1]
    num_non_empty_dims = len(non_empty_dims)

    interpretable_as_image = True
    # Catch some errors early:
    if num_non_empty_dims < 2 or 3 < num_non_empty_dims:
        _send_warning(f"Expected image, got array of shape {shape}", 1)
        interpretable_as_image = False

    if len(shape) == 3:
        depth = shape[2]
        if depth not in (1, 3, 4):
            _send_warning(
                f"Expected image depth of 1 (gray), 3 (RGB) or 4 (RGBA). Instead got array of shape {shape}", 1
            )
            interpretable_as_image = False

    needs_dim_squeeze = interpretable_as_image and num_non_empty_dims != len(shape)

    _log_tensor(obj_path, image, timeless=timeless, squeeze_dims=needs_dim_squeeze)


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
    shape = _get_image_shape(image)
    non_empty_dims = [d for d in shape if d != 1]
    num_non_empty_dims = len(non_empty_dims)

    # Catch some errors early:
    if num_non_empty_dims != 2:
        _send_warning(f"Expected 2D depth image, got array of shape {shape}", 1)
        _log_tensor(obj_path, image, timeless=timeless)
    else:
        needs_dim_squeeze = num_non_empty_dims != len(shape)
        _log_tensor(obj_path, image, meter=meter, timeless=timeless, squeeze_dims=needs_dim_squeeze)


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
    non_empty_dims = [d for d in image.shape if d != 1]
    num_non_empty_dims = len(non_empty_dims)

    # Catch some errors early:
    if num_non_empty_dims != 2:
        _send_warning(
            f"Expected single channel image, got array of shape {image.shape}. Can't interpret as segmentation image.",
            1,
        )
        _log_tensor(
            obj_path,
            tensor=image,
            timeless=timeless,
        )
    else:
        needs_dim_squeeze = num_non_empty_dims != len(image.shape)
        _log_tensor(
            obj_path,
            tensor=image,
            meaning=bindings.TensorDataMeaning.ClassId,
            timeless=timeless,
            squeeze_dims=needs_dim_squeeze,
        )
