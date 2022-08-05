# The Rerun Python SDK, which is a wrapper around the Rust crate rerun_sdk
import numpy as np

from .rerun_sdk import *

print("rerun_sdk initialized")


def log_points(name, positions, colors):
    if colors is not None:
        # Rust expects colors in 0-255 uint8
        if colors.dtype in ['float32', 'float64']:
            max = np.amax(colors)
            if max < 1.1:
                # TODO(emilk): gamma curve correction for RGB, and just *255 for alpgha
                raise TypeError(
                    "Expected color values in 0-255 gamma range, but got color values in 0-1 range")

        colors = colors.astype('uint8')
        # TODO(emilk): extend colors with alpha=255 if colors is Nx3

    positions.astype('float32')

    log_points_rs(name, positions, colors)


def log_image(name, image):
    # Catch some errors early:
    if len(image.shape) < 2 or 3 < len(image.shape):
        raise TypeError(f"Expected image, got array of shape {image.shape}")

    if len(image.shape) == 3:
        depth = image.shape[2]
        if depth not in (1, 3, 4):
            raise TypeError(
                f"Expected image depth of  of 1 (gray), 3 (RGB) or 4 (RGBA), got array of shape {image.shape}")

    log_tensor(name, image)


def log_tensor(name, image):
    if image.dtype == 'uint8':
        log_tensor_u8(name, image)
    elif image.dtype == 'uint16':
        log_tensor_u16(name, image)
    elif image.dtype == 'float32':
        log_tensor_f32(name, image)
    elif image.dtype == 'float64':
        log_tensor_f32(name, image.astype('float32'))
    else:
        raise TypeError(f"Unsupported dtype: {image.dtype}")
