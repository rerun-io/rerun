# The Rerun Python SDK, which is a wrapper around the Rust crate rerun_sdk
import atexit
from enum import Enum
import numpy as np
from typing import Optional, Sequence, Union

from . import rerun_sdk as rerun_rs  # type: ignore
from .color_conversion import linear_to_gamma_u8_pixel


def rerun_shutdown():
    rerun_rs.flush()


atexit.register(rerun_shutdown)

# -----------------------------------------------------------------------------

ArrayLike = Union[np.ndarray, Sequence]


class MeshFormat(Enum):
    # Needs some way of logging materials too, or adding some default material to the viewer.
    # """ glTF """
    # GLTF = "GLTF"

    """ Binary glTF """
    GLB = "GLB"

    # Needs some way of logging materials too, or adding some default material to the viewer.
    # """ Wavefront .obj """
    # OBJ = "OBJ"


class CameraSpaceConvention(Enum):
    """The convetion used for the camera space's (3D) coordinate system."""

    # Right-handed system used by ARKit and PyTorch3D.
    # * +X = right
    # * +Y = up
    # * +Z = back(camera looks along - Z)
    X_RIGHT_Y_UP_Z_BACK = "XRightYUpZBack"

    # Right-handed system used by OpenCV.
    # * +X = right
    # * +Y = down
    # * +Z = forward
    X_RIGHT_Y_DOWN_Z_FWD = "XRightYDownZFwd"


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
    left_top: ArrayLike,
    width_height: ArrayLike,
    *,
    label: Optional[str] = None,
    color: Optional[Sequence[int]] = None,
    space: Optional[str] = None,
):
    """
    Log a 2D rectangle.

    `label` is an optional text to show inside the rectangle.
    `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    If no `space` is given, the space name "2D" will be used.
    """
    rerun_rs.log_rect(obj_path,
                      _to_sequence(left_top),
                      _to_sequence(width_height),
                      color,
                      label,
                      space)


def log_points(
        obj_path: str,
        positions: np.ndarray,
        *,
        colors: Optional[np.ndarray] = None,
        space: Optional[str] = None):
    """
    Log 2D or 3D points, with optional colors.

    `positions`: Nx2 or Nx3 array

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
        colors = np.require(colors, dtype='uint8')

    positions = np.require(positions, dtype='float32')

    rerun_rs.log_points(obj_path, positions, colors, space)


def log_camera(obj_path: str,
               rotation_q: ArrayLike,
               position: ArrayLike,
               intrinsics: ArrayLike,
               resolution: ArrayLike,
               camera_space_convention: CameraSpaceConvention = CameraSpaceConvention.X_RIGHT_Y_DOWN_Z_FWD,
               space: Optional[str] = None,):
    """Log a perspective camera model.

    `rotation_q`: array with quaternion coordinates [x, y, z, w] for the rotation from camera to world space
    `position`: array with [x, y, z] position of the camera in world space.
    `intrinsics`: row-major intrinsics matrix for projecting from camera space to pixel space
    `resolution`: array with [width, height] image resolution in pixels.
    `camera_space_convention`: The convention used for the orientation of the camera´s 3D coordinate system.

    If no `space` is given, the space name "3D" will be used.
    """
    rerun_rs.log_camera(
        obj_path,
        _to_sequence(resolution),
        _to_transposed_sequence(intrinsics),
        _to_sequence(rotation_q),
        _to_sequence(position),
        camera_space_convention.value,
        space)


def log_path(
        obj_path: str,
        positions: np.ndarray,
        *,
        stroke_width: Optional[float] = None,
        color: Optional[Sequence[int]] = None,
        space: Optional[str] = None):
    """
    Log a 3D path.
    A path is a list of points connected by line segments.
    It can be used to draw approximations of smooth curves.

    The points will be connected in order, like so:

           2------3     5
          /        \   /
    0----1          \ /
                     4

    `positions`: a Nx3 array of points along the path.
    `stroke_width`: width of the line.
    `color` is optional RGB or RGBA triplet in 0-255 sRGB.

    If no `space` is given, the space name "3D" will be used.
    """
    positions = np.require(positions, dtype='float32')
    rerun_rs.log_path(obj_path, positions, stroke_width, color, space)


def log_line_segments(
        obj_path: str,
        positions: np.ndarray,
        *,
        stroke_width: Optional[float] = None,
        color: Optional[Sequence[int]] = None,
        space: Optional[str] = None):
    """
    Log many 2D or 3D line segments.

    The points will be connected in even-odd pairs, like so:

           2------3     5
                       /
    0----1            /
                     4

    `positions`: a Nx3 array of points along the path.
    `stroke_width`: width of the line.
    `color` is optional RGB or RGBA triplet in 0-255 sRGB.

    If no `space` is given, the space name "3D" will be used.
    """
    positions = np.require(positions, dtype='float32')
    rerun_rs.log_line_segments(obj_path, positions, stroke_width, color, space)


def log_image(obj_path: str, image: np.ndarray, *, space: Optional[str] = None):
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


def log_depth_image(obj_path: str, image: np.ndarray, *, meter: Optional[float] = None, space: Optional[str] = None):
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


def _log_tensor(obj_path: str, tensor: np.ndarray, *, meter: Optional[float] = None, space: Optional[str] = None):
    """
    If no `space` is given, the space name "2D" will be used.
    """
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


def log_mesh_file(obj_path: str, mesh_format: MeshFormat, mesh_file: bytes, *, transform: np.ndarray = None, space: Optional[str] = None):
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
        transform = np.require(transform, dtype='float32')

    rerun_rs.log_mesh_file(obj_path, mesh_format.value,
                           mesh_file, transform, space)


def _to_sequence(array: ArrayLike) -> Sequence:
    if hasattr(array, 'tolist'):
        return array.tolist()  # type: ignore
    return array  # type: ignore


def _to_transposed_sequence(array: ArrayLike) -> Sequence:
    return np.asarray(array).T.tolist()
