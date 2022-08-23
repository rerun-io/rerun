# The Rerun Python SDK, which is a wrapper around the Rust crate rerun_sdk
import atexit
import numpy as np
from typing import Optional, Sequence

from . import rerun_sdk as rerun_rs


def rerun_shutdown():
    rerun_rs.flush()


atexit.register(rerun_shutdown)


# TODO(emilk): remove the forwarded calls below and just import them from the rust library
# (which already has documentation etc).
# I couldn't figure out how to get Python to do this, because Python imports confuses me.


def connect(addr: Optional[str] = None):
    """ Connect to a remote rerun viewer on the given ip:port. """
    return rerun_rs.connect(addr)


def disconnect():
    return rerun_rs.disconnect()




def show():
    return rerun_rs.show()


def set_time_sequence(time_source: str, sequence: Optional[int]):
    """
    Set the current time globally. Used for all subsequent logging,
    until the next call to `set_time_sequence`.

    For instance: `set_time_sequence("frame_nr", frame_nr)`.

    You can remove a time source again using `set_time_sequence("frame_nr", None)`.
    """
    return rerun_rs.set_time_sequence(time_source, sequence)


def set_time_seconds(time_source: str, seconds: Optional[float]):
    """
    Set the current time globally. Used for all subsequent logging,
    until the next call to `set_time_seconds`.

    For instance: `set_time_seconds("capture_time", seconds_since_unix_epoch)`.

    You can remove a time source again using `set_time_seconds("capture_time", None)`.

    The argument should be in seconds, and should be measured either from the
    unix epoch (1970-01-01), or from some recent time (e.g. your program startup).

    The rerun_sdk has a built-in time which is `log_time`, and is logged as seconds since unix epoch.
    """
    return rerun_rs.set_time_seconds(time_source, seconds)


def set_time_nanos(time_source: str, nanos: Optional[int]):
    """
    Set the current time globally. Used for all subsequent logging,
    until the next call to `set_time_nanos`.

    For instance: `set_time_nanos("capture_time", nanos_since_unix_epoch)`.

    You can remove a time source again using `set_time_nanos("capture_time", None)`.

    The argument should be in nanoseconds, and should be measured either from the
    unix epoch (1970-01-01), or from some recent time (e.g. your program startup).

    The rerun_sdk has a built-in time which is `log_time`, and is logged as nanos since unix epoch.
    """
    return rerun_rs.set_time_nanos(time_source, nanos)


def log_rect(
    obj_path: str,
    left_top: Sequence[float],
    width_height: Sequence[float],
    label: Optional[str] = None,
    space: Optional[str] = None,
):
    """
    Log a 2D rectangle.

    Optionally give it a label and space.
    If no `space` is given, the space name "2D" will be used.
    """
    rerun_rs.log_rect(obj_path,
                      left_top,
                      width_height,
                      label,
                      space)


def log_points(
        obj_path: str,
        positions: np.ndarray,
        colors: Optional[np.ndarray] = None,
        space: Optional[str] = None):
    """
    Log 2D or 3D points, with optional colors.

    positions: Nx2 or Nx3 array

    `colors.shape[0] == 1`: same color for all points
    `colors.shape[0] == positions.shape[0]`: a color per point

    If no `space` is given, the space name "2D" or "3D" will be used,
    depending on the dimensionality of the data.
    """
    if colors is None:
        # An empty array represents no colors.
        colors = np.array((), dtype=np.uint8)
    else:
        # Rust expects colors in 0-255 uint8
        if colors.dtype in ['float32', 'float64']:
            if np.amax(colors) < 1.1:
                # TODO(emilk): gamma curve correction for RGB, and just *255 for alpgha
                raise TypeError(
                    "Expected color values in 0-255 gamma range, but got color values in 0-1 range")

        colors = colors.astype('uint8')
        # TODO(emilk): extend colors with alpha=255 if colors is Nx3

    positions.astype('float32')

    # Workaround to handle that `rerun_rs` can't handle numpy views correctly.
    # TODO(nikolausWest): Remove this extra copy once underlying issue in Rust SDK is fixed.
    positions = positions if positions.base is None else positions.copy()

    rerun_rs.log_points_rs(obj_path, positions, colors, space)


def log_image(obj_path: str, image: np.ndarray, space: Optional[str] = None):
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

    log_tensor(obj_path, image, space=space)


def log_depth_image(obj_path: str, image: np.ndarray, meter: Optional[float] = None, space: Optional[str] = None):
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

    log_tensor(obj_path, image, meter=meter, space=space)


def log_tensor(obj_path: str, tensor: np.ndarray, meter: Optional[float] = None, space: Optional[str] = None):
    """
    If no `space` is given, the space name "2D" will be used.
    """
    # Workaround to handle that `rerun_rs` can't handle numpy views correctly.
    # TODO(nikolausWest): Remove this extra copy once underlying issue in Rust SDK is fixed.
    tensor = tensor if tensor.base is None else tensor.copy()

    if tensor.dtype == 'uint8':
        rerun_rs.log_tensor_u8(obj_path, tensor, meter, space)
    elif tensor.dtype == 'uint16':
        rerun_rs.log_tensor_u16(obj_path, tensor, meter, space)
    elif tensor.dtype == 'float32':
        rerun_rs.log_tensor_f32(obj_path, tensor, meter, space)
    elif tensor.dtype == 'float64':
        rerun_rs.log_tensor_f32(
            obj_path, tensor.astype('float32'), meter, space)
    else:
        raise TypeError(f"Unsupported dtype: {tensor.dtype}")
