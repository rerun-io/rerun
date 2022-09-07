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
    # * +Z = back (camera looks along -Z)
    X_RIGHT_Y_UP_Z_BACK = "XRightYUpZBack"

    # Right-handed system used by OpenCV.
    # * +X = right
    # * +Y = down
    # * +Z = forward
    X_RIGHT_Y_DOWN_Z_FWD = "XRightYDownZFwd"


def get_recording_id() -> str:
    """
    Get the recording ID that this process is logging to, as a UUIDv4.

    The default recording_id is based on `multiprocessing.current_process().authkey`
    which means that all processes spawned with `multiprocessing`
    will have the same default recording_id.

    If you are not using `multiprocessing` and still want several different Python processes
    to log to the same Rerun instance (and be part of the same recording),
    you will need to manually assign them all the same recording_id.
    Any random UUIDv4 will work, or copy the recording id for the parent process.
    """
    return rerun_rs.get_recording_id()

def set_recording_id(value: str):
    """
    Set the recording ID that this process is logging to, as a UUIDv4.

    The default recording_id is based on `multiprocessing.current_process().authkey`
    which means that all processes spawned with `multiprocessing`
    will have the same default recording_id.

    If you are not using `multiprocessing` and still want several different Python processes
    to log to the same Rerun instance (and be part of the same recording),
    you will need to manually assign them all the same recording_id.
    Any random UUIDv4 will work, or copy the recording id for the parent process.
    """
    rerun_rs.set_recording_id(str)


def connect(addr: Optional[str] = None):
    """
    Connect to a remote rerun viewer on the given ip:port.
    """
    return rerun_rs.connect(addr)


def disconnect():
    """ Disconnect from the remote rerun server (if any). """
    return rerun_rs.disconnect()


def show():
    """
    Show previously logged data.

    This only works if you have not called `connect`.

    This will clear the logged data after showing it.

    NOTE: There is a bug which causes this function to only work once on some platforms.
    """
    return rerun_rs.show()


def save(path: str):
    """
    Save previously logged data to a file.

    This only works if you have not called `connect`.

    This will clear the logged data after saving.
    """
    return rerun_rs.save(path)


def set_time_sequence(time_source: str, sequence: Optional[int]):
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_sequence`.

    For instance: `set_time_sequence("frame_nr", frame_nr)`.

    You can remove a time source again using `set_time_sequence("frame_nr", None)`.
    """
    return rerun_rs.set_time_sequence(time_source, sequence)


def set_time_seconds(time_source: str, seconds: Optional[float]):
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
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
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
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


# """ How to specify rectangles (axis-aligned bounding boxes). """
class RectFormat(Enum):
    # """ [x,y,w,h], with x,y = left,top. """"
    XYWH = "XYWH"

    # """ [y,x,h,w], with x,y = left,top. """"
    YXHW = "YXHW"

    # """ [x0, y0, x1, y1], with x0,y0 = left,top and x1,y1 = right,bottom """"
    XYXY = "XYXY"

    # """ [y0, x0, y1, x1], with x0,y0 = left,top and x1,y1 = right,bottom """"
    YXYX = "YXYX"

    # """ [x_center, y_center, width, height]"
    XCYCWH = "XCYCWH"

    # """ [x_center, y_center, width/2, height/2]"
    XCYCW2H2 = "XCYCW2H2"


def log_rect(
    obj_path: str,
    rect: ArrayLike,
    *,
    rect_format: RectFormat = RectFormat.XYWH,
    color: Optional[Sequence[int]] = None,
    label: Optional[str] = None,
    space: Optional[str] = None,
):
    """
    Log a 2D rectangle.

    * `rect`: the recangle in [x, y, w, h], or some format you pick with the `rect_format` argument.
    * `rect_format`: how to interpret the `rect` argument
    * `label` is an optional text to show inside the rectangle.
    * `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    If no `space` is given, the space name "2D" will be used.
    """

    rerun_rs.log_rect(obj_path,
                      rect_format.value,
                      _to_sequence(rect),
                      color,
                      label,
                      space)


def log_rects(
    obj_path: str,
    rects: ArrayLike,
    *,
    rect_format: RectFormat = RectFormat.XYWH,
    colors: Optional[np.ndarray] = None,
    labels: Optional[Sequence[str]] = None,
    space: Optional[str] = None,
):
    """
    Log multiple 2D rectangles.

    * `rects`: Nx4 numpy array, where each row is [x, y, w, h], or some format you pick with the `rect_format` argument.
    * `rect_format`: how to interpret the `rect` argument
    * `labels` is an optional per-rectangle text to show inside the rectangle.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `colors`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear space.
    * float32/float64: all color components should be in 0-1 linear space.

    If no `space` is given, the space name "2D" will be used.
    """
    rects = np.require(rects, dtype='float32')
    colors = _normalize_colors(colors)
    if labels is None:
        labels = []

    rerun_rs.log_rects(obj_path,
                       rect_format.value,
                       rects,
                       colors,
                       labels,
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
    positions = np.require(positions, dtype='float32')
    colors = _normalize_colors(colors)

    rerun_rs.log_points(obj_path, positions, colors, space)


def _normalize_colors(colors: Optional[np.ndarray] = None):
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
    return colors


def log_camera(obj_path: str,
               rotation_q: ArrayLike,
               position: ArrayLike,
               intrinsics: ArrayLike,
               resolution: ArrayLike,
               camera_space_convention: CameraSpaceConvention = CameraSpaceConvention.X_RIGHT_Y_DOWN_Z_FWD,
               space: Optional[str] = None,
               target_space: Optional[str] = None):
    """Log a perspective camera model.

    `rotation_q`: Array with quaternion coordinates [x, y, z, w] for the rotation from camera to world space
    `position`: Array with [x, y, z] position of the camera in world space.
    `intrinsics`: Row-major intrinsics matrix for projecting from camera space to image space
    `resolution`: Array with [width, height] image resolution in pixels.
    `camera_space_convention`: The convention used for the orientation of the camera's 3D coordinate system.
    `space`: The 3D space the camera is in. Will default to "3D".
    `target_space`: The 2D space that the camera projects into.
    """
    rerun_rs.log_camera(
        obj_path,
        _to_sequence(resolution),
        _to_transposed_sequence(intrinsics),
        _to_sequence(rotation_q),
        _to_sequence(position),
        camera_space_convention.value,
        space,
        target_space)


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

    `transform` is an optional 3x4 affine transform matrix applied to the mesh.

    Example:
    ```
    # Move mesh 10 units along the X axis.
    transform=np.array([
        [1, 0, 0, 10],
        [0, 1, 0, 0],
        [0, 0, 1, 0]])
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
