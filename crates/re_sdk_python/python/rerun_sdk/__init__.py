# The Rerun Python SDK, which is a wrapper around the Rust crate rerun_sdk
import atexit
import numpy as np

from . import rerun_sdk as rerun_rs


def rerun_shutdown():
    rerun_rs.flush()


atexit.register(rerun_shutdown)


def connect_remote():
    return rerun_rs.connect_remote()


def info():
    return rerun_rs.info()


def log_bbox(
    obj_path,
    left_top,
    width_height,
    label=None,
    space=None,
):
    rerun_rs.log_bbox(obj_path,
                      left_top,
                      width_height,
                      label,
                      space)


def log_points(obj_path, positions, colors=None, space=None):
    """
    Log 2D or 3D points, with optional colors.

    positions: Nx2 or Nx3 array

    `colors.shape[0] == 1`: same color for all points
    `colors.shape[0] == positions.shape[0]`: a color per point

    If no `space` is given, the space name "2D" or "3D" will be used,
    depending on the dimensionality of the data.
    """
    if colors is not None:
        # Rust expects colors in 0-255 uint8
        if colors.dtype in ['float32', 'float64']:
            if np.amax(colors) < 1.1:
                # TODO(emilk): gamma curve correction for RGB, and just *255 for alpgha
                raise TypeError(
                    "Expected color values in 0-255 gamma range, but got color values in 0-1 range")

        colors = colors.astype('uint8')
        # TODO(emilk): extend colors with alpha=255 if colors is Nx3

    positions.astype('float32')

    rerun_rs.log_points_rs(obj_path, positions, colors, space)


def log_image(obj_path, image, space=None):
    """
    Log an image with 1, 3 or 4 channels (gray, RGB or RGBA).

    If no `space` is given, the space name "2D" will be used.
    """
    # Catch some errors early:
    if len(image.shape) < 2 or 3 < len(image.shape):
        raise TypeError(f"Expected image, got array of shape {image.shape}")

    if len(image.shape) == 3:
        depth = image.shape[2]
        if depth not in (1, 3, 4):
            raise TypeError(
                f"Expected image depth of 1 (gray), 3 (RGB) or 4 (RGBA). Instead got array of shape {image.shape}")

    log_tensor(obj_path, image, space)


def log_depth_image(obj_path, image, meter=None, space=None):
    """
    meter: How long is a meter in the given dtype?
           For instance: with uint16, perhaps meter=1000 which would mean
           you have millimeter precision and a range of up to ~65 meters (2^16 / 1000).

    If no `space` is given, the space name "2D" will be used.
    """
    # Catch some errors early:
    if len(image.shape) != 2:
        raise TypeError(
            f"Expected 2D depth image, got array of shape {image.shape}")

    log_tensor(obj_path, image, space)

    if meter is not None:
        rerun_rs.log_f32(obj_path, "meter", meter)


def log_tensor(obj_path, image, space=None):
    """
    If no `space` is given, the space name "2D" will be used.
    """
    if image.dtype == 'uint8':
        rerun_rs.log_tensor_u8(obj_path, image, space)
    elif image.dtype == 'uint16':
        rerun_rs.log_tensor_u16(obj_path, image, space)
    elif image.dtype == 'float32':
        rerun_rs.log_tensor_f32(obj_path, image, space)
    elif image.dtype == 'float64':
        rerun_rs.log_tensor_f32(obj_path, image.astype('float32'), space)
    else:
        raise TypeError(f"Unsupported dtype: {image.dtype}")
