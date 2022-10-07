"""The Rerun Python SDK, which is a wrapper around the Rust crate rerun_sdk."""

import atexit
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Final, Iterable, Optional, Sequence, Union

import numpy as np
import numpy.typing as npt
from rerun_sdk import rerun_sdk as rerun_rs  # type: ignore[attr-defined]
from rerun_sdk.color_conversion import linear_to_gamma_u8_pixel


def rerun_shutdown() -> None:
    rerun_rs.flush()


atexit.register(rerun_shutdown)

# -----------------------------------------------------------------------------

# ArrayLike = Union[np.ndarray, Sequence]
ColorDtype = Union[np.uint8, np.float32, np.float64]
Colors = npt.NDArray[ColorDtype]


class MeshFormat(Enum):
    # Needs some way of logging materials too, or adding some default material to the
    # viewer.
    # GLTF = "GLTF"

    # Binary glTF
    GLB = "GLB"

    # Needs some way of logging materials too, or adding some default material to the
    # viewer.
    # Wavefront .obj
    OBJ = "OBJ"


class CameraSpaceConvention(Enum):
    """The convention used for the camera space's (3D) coordinate system."""

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


@dataclass
class ImageFormat(Enum):
    # """ jpeg """"
    JPEG = "jpeg"


def get_recording_id() -> str:
    """
    Get the recording ID that this process is logging to, as a UUIDv4.

    The default recording_id is based on `multiprocessing.current_process().authkey`
    which means that all processes spawned with `multiprocessing`
    will have the same default recording_id.

    If you are not using `multiprocessing` and still want several different Python
    processes to log to the same Rerun instance (and be part of the same recording),
    you will need to manually assign them all the same recording_id.
    Any random UUIDv4 will work, or copy the recording id for the parent process.
    """
    return str(rerun_rs.get_recording_id())


def set_recording_id(value: str) -> None:
    """
    Set the recording ID that this process is logging to, as a UUIDv4.

    The default recording_id is based on `multiprocessing.current_process().authkey`
    which means that all processes spawned with `multiprocessing`
    will have the same default recording_id.

    If you are not using `multiprocessing` and still want several different Python
    processes to log to the same Rerun instance (and be part of the same recording),
    you will need to manually assign them all the same recording_id.
    Any random UUIDv4 will work, or copy the recording id for the parent process.
    """
    rerun_rs.set_recording_id(str)


def connect(addr: Optional[str] = None) -> None:
    """Connect to a remote Rerun Viewer on the given ip:port."""
    rerun_rs.connect(addr)


def serve() -> None:
    """
    Serve a Rerun Web Viewer.

    WARNING: This is an experimental feature.
    """
    rerun_rs.serve()


def disconnect() -> None:
    """Disconnect from the remote rerun server (if any)."""
    rerun_rs.disconnect()


def show() -> None:
    """
    Show previously logged data.

    This only works if you have not called `connect`.

    This will clear the logged data after showing it.

    NOTE: There is a bug which causes this function to only work once on some platforms.
    """
    rerun_rs.show()


def save(path: str) -> None:
    """
    Save previously logged data to a file.

    This only works if you have not called `connect`.

    This will clear the logged data after saving.
    """
    rerun_rs.save(path)


def set_time_sequence(time_source: str, sequence: Optional[int]) -> None:
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_sequence`.

    For instance: `set_time_sequence("frame_nr", frame_nr)`.

    You can remove a time source again using `set_time_sequence("frame_nr", None)`.

    There is no requirement of monoticity. You can move the time backwards if you like.
    """
    rerun_rs.set_time_sequence(time_source, sequence)


def set_time_seconds(time_source: str, seconds: Optional[float]) -> None:
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_seconds`.

    For instance: `set_time_seconds("capture_time", seconds_since_unix_epoch)`.

    You can remove a time source again using `set_time_seconds("capture_time", None)`.

    The argument should be in seconds, and should be measured either from the
    unix epoch (1970-01-01), or from some recent time (e.g. your program startup).

    The rerun_sdk has a built-in time which is `log_time`, and is logged as seconds
    since unix epoch.

    There is no requirement of monoticity. You can move the time backwards if you like.
    """
    rerun_rs.set_time_seconds(time_source, seconds)


def set_time_nanos(time_source: str, nanos: Optional[int]) -> None:
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_nanos`.

    For instance: `set_time_nanos("capture_time", nanos_since_unix_epoch)`.

    You can remove a time source again using `set_time_nanos("capture_time", None)`.

    The argument should be in nanoseconds, and should be measured either from the
    unix epoch (1970-01-01), or from some recent time (e.g. your program startup).

    The rerun_sdk has a built-in time which is `log_time`, and is logged as nanos since
    unix epoch.

    There is no requirement of monoticity. You can move the time backwards if you like.
    """
    rerun_rs.set_time_nanos(time_source, nanos)


def set_space_up(space: str, up: Sequence[float]) -> None:
    """
    Set the preferred up-axis in the viewer for a given 3D space.

    - space: The name of the space
    - up: The (x, y, z) values of the up-axis
    """
    rerun_rs.set_space_up(space, up)


@dataclass
class LogLevel:
    """
    Represents the standard log levels.

    This is a collection of constants rather than an enum because we do support
    arbitrary strings as level (e.g. for user-defined levels).
    """

    # """ Designates very serious errors. """
    ERROR: Final = "ERROR"
    # """ Designates hazardous situations. """
    WARN: Final = "WARN"
    # """ Designates useful information. """
    INFO: Final = "INFO"
    # """ Designates lower priority information. """
    DEBUG: Final = "DEBUG"
    # """ Designates very low priority, often extremely verbose, information. """
    TRACE: Final = "TRACE"


def log_text_entry(
    obj_path: str,
    text: str,
    level: Optional[str] = LogLevel.INFO,
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    """
    Log a text entry, with optional level.

    * If no `level` is given, it will default to `LogLevel.INFO`.
    * `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    * If no `space` is given, the space name "logs" will be used.
    """
    rerun_rs.log_text_entry(obj_path, text, level, color, timeless, space)


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
    rect: npt.ArrayLike,
    *,
    rect_format: RectFormat = RectFormat.XYWH,
    color: Optional[Sequence[int]] = None,
    label: Optional[str] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    """
    Log a 2D rectangle.

    * `rect`: the recangle in [x, y, w, h], or some format you pick with the
    `rect_format` argument.
    * `rect_format`: how to interpret the `rect` argument
    * `label` is an optional text to show inside the rectangle.
    * `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    If no `space` is given, the space name "2D" will be used.
    """
    rerun_rs.log_rect(obj_path, rect_format.value, _to_sequence(rect), color, label, timeless, space)


def log_rects(
    obj_path: str,
    rects: npt.ArrayLike,
    *,
    rect_format: RectFormat = RectFormat.XYWH,
    colors: Optional[Colors] = None,
    labels: Optional[Sequence[str]] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    """
    Log multiple 2D rectangles.

    Logging again to the same `obj_path` will replace all the previous rectangles.

    * `rects`: Nx4 numpy array, where each row is [x, y, w, h], or some format you pick with the `rect_format`
    argument.
    * `rect_format`: how to interpret the `rect` argument
    * `labels` is an optional per-rectangle text to show inside the rectangle.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `colors`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
    * float32/float64: all color components should be in 0-1 linear space.

    If no `space` is given, the space name "2D" will be used.
    """
    rects = np.require(rects, dtype="float32")
    colors = _normalize_colors(colors)
    if labels is None:
        labels = []

    rerun_rs.log_rects(obj_path, rect_format.value, rects, colors, labels, timeless, space)


def log_point(
    obj_path: str,
    position: npt.NDArray[np.float32],
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    """
    Log a 2D or 3D point, with optional color.

    Logging again to the same `obj_path` will replace the previous point.

    `position`: 2x1 or 3x1 array

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `colors`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
    * float32/float64: all color components should be in 0-1 linear space.

    If no `space` is given, the space name "2D" or "3D" will be used,
    depending on the dimensionality of the data.
    """
    position = np.require(position, dtype="float32")
    rerun_rs.log_point(obj_path, position, color, timeless, space)


def log_points(
    obj_path: str,
    positions: npt.NDArray[np.float32],
    *,
    colors: Optional[Colors] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    """
    Log 2D or 3D points, with optional colors.

    Logging again to the same `obj_path` will replace all the previous points.

    `positions`: Nx2 or Nx3 array

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `colors`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
    * float32/float64: all color components should be in 0-1 linear space.

    If no `space` is given, the space name "2D" or "3D" will be used,
    depending on the dimensionality of the data.
    """
    positions = np.require(positions, dtype="float32")
    colors = _normalize_colors(colors)

    rerun_rs.log_points(obj_path, positions, colors, timeless, space)


def _normalize_colors(colors: Optional[npt.ArrayLike] = None) -> npt.NDArray[np.uint8]:
    """Normalize flexible colors arrays."""
    if colors is None:
        # An empty array represents no colors.
        return np.array((), dtype=np.uint8)
    else:
        colors_array = np.array(colors)

        # Rust expects colors in 0-255 uint8
        if colors_array.dtype.type in [np.float32, np.float64]:
            return linear_to_gamma_u8_pixel(linear=colors_array)

        return np.require(colors_array, np.uint8)


def log_camera(
    obj_path: str,
    rotation_q: npt.ArrayLike,
    position: npt.ArrayLike,
    intrinsics: npt.ArrayLike,
    resolution: npt.ArrayLike,
    camera_space_convention: CameraSpaceConvention = CameraSpaceConvention.X_RIGHT_Y_DOWN_Z_FWD,
    timeless: bool = False,
    space: Optional[str] = None,
    target_space: Optional[str] = None,
) -> None:
    """
    Log a perspective camera model.

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
        resolution=_to_sequence(resolution),
        intrinsics_matrix=np.asarray(intrinsics).T.tolist(),
        rotation_q=_to_sequence(rotation_q),
        position=_to_sequence(position),
        camera_space_convention=camera_space_convention.value,
        timeless=timeless,
        space=space,
        target_space=target_space,
    )


def _log_extrinsics(
    obj_path: str,
    rotation_q: npt.ArrayLike,
    position: npt.ArrayLike,
    camera_space_convention: CameraSpaceConvention = CameraSpaceConvention.X_RIGHT_Y_DOWN_Z_FWD,
    timeless: bool = False,
) -> None:
    """
    EXPERIMENTAL: Log camera extrinsics.

    `rotation_q`: Array with quaternion coordinates [x, y, z, w] for the rotation from camera to world space
    `position`: Array with [x, y, z] position of the camera in world space.
    `camera_space_convention`: The convention used for the orientation of the camera's 3D coordinate system.
    """
    rerun_rs.log_extrinsics(
        obj_path,
        rotation=_to_sequence(rotation_q),
        position=_to_sequence(position),
        camera_space_convention=camera_space_convention.value,
        timeless=timeless,
    )


def _log_intrinsics(
    obj_path: str, intrinsics: npt.ArrayLike, resolution: npt.ArrayLike, timeless: bool = False
) -> None:
    """
    EXPERIMENTAL: Log a perspective camera model.

    `intrinsics`: Row-major intrinsics matrix for projecting from camera space to image space
    `resolution`: Array with [width, height] image resolution in pixels.
    """
    rerun_rs.log_intrinsics(
        obj_path, resolution=_to_sequence(resolution), intrinsics=np.asarray(intrinsics).T.tolist(), timeless=timeless
    )


def log_path(
    obj_path: str,
    positions: npt.NDArray[np.float32],
    *,
    stroke_width: Optional[float] = None,
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    r"""
    Log a 3D path.

    A path is a list of points connected by line segments. It can be used to draw approximations of smooth curves.

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
    positions = np.require(positions, dtype="float32")
    rerun_rs.log_path(obj_path, positions, stroke_width, color, timeless, space)


def log_line_segments(
    obj_path: str,
    positions: npt.NDArray[np.float32],
    *,
    stroke_width: Optional[float] = None,
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    r"""
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
    positions = np.require(positions, dtype="float32")
    rerun_rs.log_line_segments(obj_path, positions, stroke_width, color, timeless, space)


def log_obb(
    obj_path: str,
    half_size: npt.ArrayLike,
    position: npt.ArrayLike,
    rotation_q: npt.ArrayLike,
    color: Optional[Sequence[int]] = None,
    stroke_width: Optional[float] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    """
    Log a 3D oriented bounding box, defined by its half size.

    `half_size`: Array with [x, y, z] half dimensions of the OBB.
    `position`: Array with [x, y, z] position of the OBB in world space.
    `rotation_q`: Array with quaternion coordinates [x, y, z, w] for the rotation from model to world space
    `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    `stroke_width`: width of the OBB edges.
    `space`: The 3D space the OBB is in. Will default to "3D".
    """
    rerun_rs.log_obb(
        obj_path,
        _to_sequence(half_size),
        _to_sequence(position),
        _to_sequence(rotation_q),
        color,
        stroke_width,
        timeless,
        space,
    )


def log_image(
    obj_path: str,
    image: Colors,
    *,
    timeless: bool = False,
    space: Optional[str] = None,
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

    If no `space` is given, the space name "2D" will be used.
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

    log_tensor(obj_path, image, timeless=timeless, space=space)


def log_depth_image(
    obj_path: str,
    image: Colors,
    *,
    meter: Optional[float] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
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
        raise TypeError(f"Expected 2D depth image, got array of shape {image.shape}")

    log_tensor(obj_path, image, meter=meter, timeless=timeless, space=space)


def log_tensor(
    obj_path: str,
    tensor: npt.NDArray[Union[np.uint8, np.uint16, np.float32, np.float64]],
    names: Optional[Iterable[str]] = None,
    meter: Optional[float] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    """If no `space` is given, the space name "2D" will be used."""
    if names is not None:
        names = list(names)
        assert len(tensor.shape) == len(names)

    if tensor.dtype == "uint8":
        rerun_rs.log_tensor_u8(obj_path, tensor, names, meter, timeless, space)
    elif tensor.dtype == "uint16":
        rerun_rs.log_tensor_u16(obj_path, tensor, names, meter, timeless, space)
    elif tensor.dtype == "float32":
        rerun_rs.log_tensor_f32(obj_path, tensor, names, meter, timeless, space)
    elif tensor.dtype == "float64":
        rerun_rs.log_tensor_f32(obj_path, tensor.astype("float32"), names, meter, timeless, space)
    else:
        raise TypeError(f"Unsupported dtype: {tensor.dtype}")


def log_mesh_file(
    obj_path: str,
    mesh_format: MeshFormat,
    mesh_file: bytes,
    *,
    transform: Optional[npt.NDArray[np.float32]] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    """
    Log the contents of a mesh file (.gltf, .glb, .obj, …).

    `transform` is an optional 3x4 affine transform matrix applied to the mesh.

    Example:
    -------
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
        transform = np.require(transform, dtype="float32")

    rerun_rs.log_mesh_file(obj_path, mesh_format.value, mesh_file, transform, timeless, space)


def log_image_file(
    obj_path: str,
    img_path: Path,
    img_format: Optional[ImageFormat] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    """
    Log the contents of an image file (only JPEGs supported for now).

    If no `img_format` is specified, we will try and guess it.
    If no `space` is given, the space name "2D" will be used.
    """
    img_format = getattr(img_format, "value", None)
    rerun_rs.log_image_file(obj_path, img_path, img_format, timeless, space)


def _to_sequence(array: npt.ArrayLike) -> Sequence[float]:
    if isinstance(array, np.ndarray):
        return np.require(array, float).tolist()  # type: ignore[no-any-return]

    return array  # type: ignore[return-value]
