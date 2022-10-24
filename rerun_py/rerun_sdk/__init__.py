"""The Rerun Python SDK, which is a wrapper around the Rust crate rerun_sdk."""

import atexit
import logging
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Final, Iterable, Optional, Sequence, Tuple, Union

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
Color = Union[npt.NDArray[ColorDtype], Sequence[Union[int, float]]]

ClassIdDtype = Union[np.uint8, np.uint16]
ClassIds = npt.NDArray[ClassIdDtype]


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


def set_time_sequence(timeline: str, sequence: Optional[int]) -> None:
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_sequence`.

    For instance: `set_time_sequence("frame_nr", frame_nr)`.

    You can remove a timeline again using `set_time_sequence("frame_nr", None)`.

    There is no requirement of monoticity. You can move the time backwards if you like.
    """
    rerun_rs.set_time_sequence(timeline, sequence)


def set_time_seconds(timeline: str, seconds: Optional[float]) -> None:
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_seconds`.

    For instance: `set_time_seconds("capture_time", seconds_since_unix_epoch)`.

    You can remove a timeline again using `set_time_seconds("capture_time", None)`.

    The argument should be in seconds, and should be measured either from the
    unix epoch (1970-01-01), or from some recent time (e.g. your program startup).

    The rerun_sdk has a built-in time which is `log_time`, and is logged as seconds
    since unix epoch.

    There is no requirement of monoticity. You can move the time backwards if you like.
    """
    rerun_rs.set_time_seconds(timeline, seconds)


def set_time_nanos(timeline: str, nanos: Optional[int]) -> None:
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_nanos`.

    For instance: `set_time_nanos("capture_time", nanos_since_unix_epoch)`.

    You can remove a timeline again using `set_time_nanos("capture_time", None)`.

    The argument should be in nanoseconds, and should be measured either from the
    unix epoch (1970-01-01), or from some recent time (e.g. your program startup).

    The rerun_sdk has a built-in time which is `log_time`, and is logged as nanos since
    unix epoch.

    There is no requirement of monoticity. You can move the time backwards if you like.
    """
    rerun_rs.set_time_nanos(timeline, nanos)


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

    # """ Designates catastrophic failures. """
    CRITICAL: Final = "CRITICAL"
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


class LoggingHandler(logging.Handler):
    """
    Provides a logging handler that forwards all events to the Rerun SDK.

    Because Rerun's data model doesn't match 1-to-1 with the different concepts from
    python's logging ecosystem, we need a way to map the latter to the former:

    * Root Object: Optional root object to gather all the logs under.

    * Object path: the name of the logger responsible for the creation of the LogRecord
                   is used as the final object path, appended after the Root Object path.

    * Level: the log level is mapped as-is.

    * Body: the body of the text entry corresponds to the formatted output of
            the LogRecord using the standard formatter of the logging package,
            unless it has been overridden by the user.

    Read more about logging handlers at https://docs.python.org/3/howto/logging.html#handlers.
    """

    LVL2NAME: Final = {
        logging.CRITICAL: LogLevel.CRITICAL,
        logging.ERROR: LogLevel.ERROR,
        logging.WARNING: LogLevel.WARN,
        logging.INFO: LogLevel.INFO,
        logging.DEBUG: LogLevel.DEBUG,
    }

    def __init__(self, root_obj_path: Optional[str] = None):
        logging.Handler.__init__(self)
        self.root_obj_path = root_obj_path

    def emit(self, record: logging.LogRecord) -> None:
        """Emits a record to the Rerun SDK."""
        objpath = record.name.replace(".", "/")
        if self.root_obj_path is not None:
            objpath = f"{self.root_obj_path}/{objpath}"
        level = self.LVL2NAME.get(record.levelno)
        if level is None:  # user-defined level
            level = record.levelname
        log_text_entry(objpath, record.getMessage(), level=level)


def log_text_entry(
    obj_path: str,
    text: str,
    level: Optional[str] = LogLevel.INFO,
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
) -> None:
    """
    Log a text entry, with optional level.

    * If no `level` is given, it will default to `LogLevel.INFO`.
    * `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    """
    rerun_rs.log_text_entry(obj_path, text, level, color, timeless)


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
) -> None:
    """
    Log a 2D rectangle.

    * `rect`: the recangle in [x, y, w, h], or some format you pick with the
    `rect_format` argument.
    * `rect_format`: how to interpret the `rect` argument
    * `label` is an optional text to show inside the rectangle.
    * `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    """
    rerun_rs.log_rect(obj_path, rect_format.value, _to_sequence(rect), color, label, timeless)


def log_rects(
    obj_path: str,
    rects: npt.ArrayLike,
    *,
    rect_format: RectFormat = RectFormat.XYWH,
    colors: Optional[Colors] = None,
    labels: Optional[Sequence[str]] = None,
    timeless: bool = False,
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

    """
    rects = np.require(rects, dtype="float32")
    colors = _normalize_colors(colors)
    if labels is None:
        labels = []

    rerun_rs.log_rects(obj_path, rect_format.value, rects, colors, labels, timeless)


def log_point(
    obj_path: str,
    position: npt.NDArray[np.float32],
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
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
    rerun_rs.log_point(obj_path, position, color, timeless)


def log_points(
    obj_path: str,
    positions: npt.NDArray[np.float32],
    *,
    colors: Optional[Colors] = None,
    timeless: bool = False,
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

    rerun_rs.log_points(obj_path, positions, colors, timeless)


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


def log_extrinsics(
    obj_path: str,
    rotation_q: npt.ArrayLike,
    position: npt.ArrayLike,
    camera_space_convention: CameraSpaceConvention = CameraSpaceConvention.X_RIGHT_Y_DOWN_Z_FWD,
    timeless: bool = False,
) -> None:
    """
    Log camera extrinsics.

    This logs a transform between this object and the parent object.

    Example
    -------
    ```
    rerun.log_extrinsics("3d/camera", …)
    rerun.log_intrinsics("3d/camera/image", …)
    ```

    `rotation_q`: Array with quaternion coordinates [x, y, z, w] for the rotation from camera to world space
    `position`: Array with [x, y, z] position of the camera in world space.
    `camera_space_convention`: The convention used for the orientation of the camera's 3D coordinate system.

    """
    rerun_rs.log_extrinsics(
        obj_path,
        rotation_q=_to_sequence(rotation_q),
        position=_to_sequence(position),
        camera_space_convention=camera_space_convention.value,
        timeless=timeless,
    )


def log_intrinsics(
    obj_path: str, *, width: int, height: int, intrinsics_matrix: npt.ArrayLike, timeless: bool = False
) -> None:
    """
    Log a perspective camera model.

    This logs a transform between this object and the parent object.

    Example
    -------
    ```
    rerun.log_extrinsics("3d/camera", …)
    rerun.log_intrinsics("3d/camera/image", …)
    ```

    `intrinsics_matrix`: Row-major intrinsics matrix for projecting from camera space to image space
    `resolution`: Array with [width, height] image resolution in pixels.

    """
    rerun_rs.log_intrinsics(
        obj_path,
        resolution=[width, height],
        intrinsics_matrix=np.asarray(intrinsics_matrix).T.tolist(),
        timeless=timeless,
    )


def log_path(
    obj_path: str,
    positions: npt.NDArray[np.float32],
    *,
    stroke_width: Optional[float] = None,
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
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
    """
    positions = np.require(positions, dtype="float32")
    rerun_rs.log_path(obj_path, positions, stroke_width, color, timeless)


def log_line_segments(
    obj_path: str,
    positions: npt.NDArray[np.float32],
    *,
    stroke_width: Optional[float] = None,
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
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
    """
    positions = np.require(positions, dtype="float32")
    rerun_rs.log_line_segments(obj_path, positions, stroke_width, color, timeless)


def log_arrow(
    obj_path: str,
    origin: npt.ArrayLike,
    vector: npt.ArrayLike,
    color: Optional[Sequence[int]] = None,
    label: Optional[str] = None,
    width_scale: Optional[float] = None,
    timeless: bool = False,
) -> None:
    """
    Log a 3D arrow.

    An arrow is defined with an `origin`, and a `vector`. This can also be considered as `start` and `end` positions
    for the arrow.

    The shaft is rendered as a cylinder with `radius = 0.5 * width_scale`.
    The tip is rendered as a cone with `height = 2.0 * width_scale` and `radius = 1.0 * width_scale`.

    Parameters
    ----------
    obj_path
        The path to store the object at.
    origin
        The base position of the arrow.
    vector
        The vector along which the arrow will be drawn.
    color
        An optional RGB or RGBA triplet in 0-255 sRGB.
    label
        An optional text to show beside the arrow.
    width_scale
        An optional scaling factor, default=1.0.
    timeless
        Object is not time-dependent, and will be visible at any time point.

    """
    rerun_rs.log_arrow(
        obj_path,
        origin=_to_sequence(origin),
        vector=_to_sequence(vector),
        color=color,
        label=label,
        width_scale=width_scale,
        timeless=timeless,
    )


def log_obb(
    obj_path: str,
    half_size: npt.ArrayLike,
    position: npt.ArrayLike,
    rotation_q: npt.ArrayLike,
    color: Optional[Sequence[int]] = None,
    stroke_width: Optional[float] = None,
    label: Optional[str] = None,
    timeless: bool = False,
) -> None:
    """
    Log a 3D oriented bounding box, defined by its half size.

    `half_size`: Array with [x, y, z] half dimensions of the OBB.
    `position`: Array with [x, y, z] position of the OBB in world space.
    `rotation_q`: Array with quaternion coordinates [x, y, z, w] for the rotation from model to world space
    `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    `stroke_width`: width of the OBB edges.
    `label` is an optional text label placed at `position`.
    """
    rerun_rs.log_obb(
        obj_path,
        half_size=_to_sequence(half_size),
        position=_to_sequence(position),
        rotation_q=_to_sequence(rotation_q),
        color=color,
        stroke_width=stroke_width,
        label=label,
        timeless=timeless,
    )


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
    image: ClassIds,
    class_descriptions: str = "",
    *,
    timeless: bool = False,
) -> None:
    """
    Log an image made up of uint8 or uint16 class-ids.

    The image should have 1 channels.

    Supported `dtype`s:
    * uint8: components should be 0-255 class ids
    * uint16: components should be 0-65535 class ids
    * class_descriptions: obj_path for a class_descriptions object logged with `log_class_descriptions`

    """
    # Catch some errors early:
    if len(image.shape) < 2 or 3 < len(image.shape):
        raise TypeError(f"Expected image, got array of shape {image.shape}")

    if len(image.shape) == 3:
        depth = image.shape[2]
        if depth != 1:
            raise TypeError(f"Expected image depth of 1. Instead got array of shape {image.shape}")

    if image.dtype == "uint8":
        rerun_rs.log_tensor_u8(obj_path, image, None, None, class_descriptions, timeless)
    elif image.dtype == "uint16":
        rerun_rs.log_tensor_u16(obj_path, image, None, None, class_descriptions, timeless)
    else:
        raise TypeError(f"Unsupported dtype: {image.dtype}")


def log_tensor(
    obj_path: str,
    tensor: npt.NDArray[Union[np.uint8, np.uint16, np.float32, np.float64]],
    names: Optional[Iterable[str]] = None,
    meter: Optional[float] = None,
    timeless: bool = False,
) -> None:
    """Log a general tensor, perhaps with named dimensions."""
    if names is not None:
        names = list(names)
        assert len(tensor.shape) == len(names)

    if tensor.dtype == "uint8":
        rerun_rs.log_tensor_u8(obj_path, tensor, names, meter, None, timeless)
    elif tensor.dtype == "uint16":
        rerun_rs.log_tensor_u16(obj_path, tensor, names, meter, None, timeless)
    elif tensor.dtype == "float32":
        rerun_rs.log_tensor_f32(obj_path, tensor, names, meter, None, timeless)
    elif tensor.dtype == "float64":
        rerun_rs.log_tensor_f32(obj_path, tensor.astype("float32"), names, meter, None, timeless)
    else:
        raise TypeError(f"Unsupported dtype: {tensor.dtype}")


def log_mesh_file(
    obj_path: str,
    mesh_format: MeshFormat,
    mesh_file: bytes,
    *,
    transform: Optional[npt.NDArray[np.float32]] = None,
    timeless: bool = False,
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

    rerun_rs.log_mesh_file(obj_path, mesh_format.value, mesh_file, transform, timeless)


def log_image_file(
    obj_path: str,
    img_path: Path,
    img_format: Optional[ImageFormat] = None,
    timeless: bool = False,
) -> None:
    """
    Log the contents of an image file (only JPEGs supported for now).

    If no `img_format` is specified, we will try and guess it.
    """
    img_format = getattr(img_format, "value", None)
    rerun_rs.log_image_file(obj_path, img_path, img_format, timeless)


def _to_sequence(array: npt.ArrayLike) -> Sequence[float]:
    if isinstance(array, np.ndarray):
        return np.require(array, float).tolist()  # type: ignore[no-any-return]

    return array  # type: ignore[return-value]


def set_visible(obj_path: str, visibile: bool) -> None:
    """Change the visibility of an object."""
    rerun_rs.set_visible(obj_path, visibile)


@dataclass
class ClassDescription:
    """
    Metadata about a class type identified by an id.

    Color and label will be used to annotate objects which reference the id.
    """

    id: int
    label: Optional[str] = None
    color: Optional[Color] = None


ClassDescriptionLike = Union[Tuple[int, str], Tuple[int, str, Color], ClassDescription]


def coerce_class_description(arg: ClassDescriptionLike) -> ClassDescription:
    if type(arg) is ClassDescription:
        return arg
    else:
        return ClassDescription(*arg)  # type: ignore[misc]


def log_class_descriptions(
    obj_path: str,
    class_descriptions: Sequence[ClassDescriptionLike],
    *,
    timeless: bool = False,
) -> None:
    """
    Log a collection of ClassDescriptions which can be used for annotation of other objects.

    This obj_path can be referenced from the `log_segmentation_image` API to
    indicate this set of descriptions is relevant to the image.

    Each ClassDescription must include an id, which will be used for matching
    the class and may optionally include a label and color.  Colors should
    either be in 0-255 gamma space or in 0-1 linear space.  Colors can be RGB or
    RGBA.

    These can either be specified verbosely as:
    ```
    [ClassDescription(id=23, label='foo', color=(255, 0, 0)), ...]
    ```

    Or using short-hand tuples.
    ```
    [(23, 'bar'), ...]
    ```

    Unspecified colors will be filled in by the visualizer randomly.
    """
    # Coerce tuples into ClassDescription dataclass for convenience
    typed_class_descriptions = (coerce_class_description(d) for d in class_descriptions)

    # Convert back to fixed tuple for easy pyo3 conversion
    tuple_class_descriptions = [
        (d.id, d.label, _normalize_colors(d.color).tolist() or None) for d in typed_class_descriptions
    ]

    rerun_rs.log_class_descriptions(obj_path, tuple_class_descriptions, timeless)
