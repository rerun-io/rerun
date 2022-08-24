# The Rerun Python SDK, which is a wrapper around the Rust crate rerun_sdk
import atexit
from enum import Enum
import numpy as np
from typing import Optional, Sequence

from . import rerun_sdk as rerun_rs  # type: ignore
from .color_conversion import linear_to_gamma_u8_pixel


def rerun_shutdown():
    rerun_rs.flush()


atexit.register(rerun_shutdown)

# -----------------------------------------------------------------------------


class MeshFormat(Enum):
    # Untested
    # """ glTF """
    # GLTF = "GLTF"

    """ Binary glTF """
    GLB = "GLB"

    # Untested
    # """ Wavefront .obj """
    # OBJ = "OBJ"


def connect(addr: Optional[str] = None):
    """ Connect to a remote rerun viewer on the given ip:port. """
    return rerun_rs.connect(addr)


def disconnect():
    """ Disconnect from the remote rerun server (if any). """
    return rerun_rs.disconnect()


def show():
    """
    Show previously logged data.

    This only works if you have not called `connect`.

    NOTE: There is a bug which causes this function to only work once on some platforms.
    """
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


def set_space_up(space: str, up: Sequence[float]):
    """ Set the preferred up-axis in the viewer for a given 3D space.

    - space: The name of the space
    - up: The (x, y, z) values of the up-axis
"""
    return rerun_rs.set_space_up(space, up)


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

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `colors`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear space.
    * float32/float64: all color components should be in 0-1 linear space.

    If no `space` is given, the space name "2D" or "3D" will be used,
    depending on the dimensionality of the data.
    """
    if colors is None:
        # An empty array represents no colors.
        colors = np.array((), dtype=np.uint8)
    else:
        # Rust expects colors in 0-255 uint8
        if colors.dtype in ['float32', 'float64']:
            colors = linear_to_gamma_u8_pixel(linear=colors)

        if colors.dtype != 'uint8':
            colors = colors.astype('uint8')

    positions.astype('float32')

    # Workaround to handle that `rerun_rs` can't handle numpy views correctly.
    # TODO(nikolausWest): Remove this extra copy once underlying issue in Rust SDK is fixed.
    positions = positions if positions.base is None else positions.copy()

    rerun_rs.log_points_rs(obj_path, positions, colors, space)


def log_image(obj_path: str, image: np.ndarray, space: Optional[str] = None):
    """
    Log a gray or color image.

    The image should either have 1, 3 or 4 channels (gray, RGB or RGBA).

    Supported `dtype`s:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear space.
    * uint16: color components should be in 0-65535 sRGB gamma space, except for alpha which should be in 0-65535 linear space.
    * float32/float64: all color components should be in 0-1 linear space.

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

    _log_tensor(obj_path, image, space=space)


def log_depth_image(obj_path: str, image: np.ndarray, meter: Optional[float] = None, space: Optional[str] = None):
    """
    Log a depth image.

    The image must be a 2D array. Supported `dtype`:s are: uint8, uint16, float32, float64

    meter: How long is a meter in the given dtype?
           For instance: with uint16, perhaps meter=1000 which would mean
           you have millimeter precision and a range of up to ~65 meters (2^16 / 1000).

    If no `space` is given, the space name "2D" will be used.
    """
    # Catch some errors early:
    if len(image.shape) != 2:
        raise TypeError(
            f"Expected 2D depth image, got array of shape {image.shape}")

    _log_tensor(obj_path, image, meter=meter, space=space)


def _log_tensor(obj_path: str, tensor: np.ndarray, meter: Optional[float] = None, space: Optional[str] = None):
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


def log_mesh_file(obj_path: str, mesh_format: MeshFormat, mesh_file: bytes, transform: np.ndarray = None, space: Optional[str] = None):
    """
    Log the contents of a mesh file (.gltf, .glb, .obj, …).

    `transform` is an optional 4x4 transform matrix applied to the mesh.

    Example:
    ```
    # Move mesh 10 units along the X axis.
    transform=np.array([
        [1, 0, 0, 10],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
        [0, 0, 0, 1]])
    ```
    """
    if transform is None:
        transform = np.empty(shape=(0, 0), dtype=np.float32)
    else:
        transform = transform.astype('float32')

    rerun_rs.log_mesh_file(obj_path, mesh_format.value,
                           mesh_file, transform, space)
