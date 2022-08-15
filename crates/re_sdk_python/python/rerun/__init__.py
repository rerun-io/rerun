# The Rerun Python SDK, which is a wrapper around a Rust crate.
import numpy as np

from .rerun import *

print("rerun initialized")


def log_points(obj_path, positions, colors):
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

    log_points_rs(obj_path, positions, colors)


def log_image(obj_path, image):
    # Catch some errors early:
    if len(image.shape) < 2 or 3 < len(image.shape):
        raise TypeError(f"Expected image, got array of shape {image.shape}")

    if len(image.shape) == 3:
        depth = image.shape[2]
        if depth not in (1, 3, 4):
            raise TypeError(
                f"Expected image depth of 1 (gray), 3 (RGB) or 4 (RGBA). Instead got array of shape {image.shape}")

    log_tensor(obj_path, image)


def log_depth_image(obj_path, image, meter=None):
    """
    meter: How long is a meter in the given dtype?
           For instance: with uint16, perhaps meter=1000 which would mean
           you have millimeter precision and a range of up to ~65 meters (2^16 / 1000).
    """
    # Catch some errors early:
    if len(image.shape) != 2:
        raise TypeError(
            f"Expected 2D depth image, got array of shape {image.shape}")

    log_tensor(obj_path, image)

    if meter != None:
        log_f32(obj_path, "meter", meter)


def log_tensor(obj_path, image):
    if image.dtype == 'uint8':
        log_tensor_u8(obj_path, image)
    elif image.dtype == 'uint16':
        log_tensor_u16(obj_path, image)
    elif image.dtype == 'float32':
        log_tensor_f32(obj_path, image)
    elif image.dtype == 'float64':
        log_tensor_f32(obj_path, image.astype('float32'))
    else:
        raise TypeError(f"Unsupported dtype: {image.dtype}")
