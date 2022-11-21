"""The Rerun Python SDK, which is a wrapper around the Rust crate rerun_sdk."""

import atexit
import logging
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Final, Iterable, Optional, Sequence, Tuple, Union

import numpy as np
import numpy.typing as npt
from rerun.color_conversion import linear_to_gamma_u8_pixel

from rerun import rerun_sdk  # type: ignore[attr-defined]


def rerun_shutdown() -> None:
    rerun_sdk.flush()


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
    return str(rerun_sdk.get_recording_id())


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
    rerun_sdk.set_recording_id(str)


def init(application_id: str) -> None:
    """
    Initialize the Rerun SDK with a user-chosen application id (name).

    Your Rerun recordings will be categorized by this application id, so
    try to pick a unique one for each application that uses the Rerun SDK.

    For instance, if you have one application doing object detection
    and another doing camera calibration, you could have
    `rerun.init("object_detector")` and `rerun.init("calibrator")`.
    """
    rerun_sdk.init(application_id)


def connect(addr: Optional[str] = None) -> None:
    """Connect to a remote Rerun Viewer on the given ip:port."""
    rerun_sdk.connect(addr)


def serve() -> None:
    """
    Serve a Rerun Web Viewer.

    WARNING: This is an experimental feature.
    """
    rerun_sdk.serve()


def disconnect() -> None:
    """Disconnect from the remote rerun server (if any)."""
    rerun_sdk.disconnect()


def show() -> None:
    """
    Show previously logged data.

    This only works if you have not called `connect`.

    This will clear the logged data after showing it.

    NOTE: There is a bug which causes this function to only work once on some platforms.
    """
    rerun_sdk.show()


def save(path: str) -> None:
    """
    Save previously logged data to a file.

    This only works if you have not called `connect`.

    This will clear the logged data after saving.
    """
    rerun_sdk.save(path)


def set_time_sequence(timeline: str, sequence: Optional[int]) -> None:
    """
    Set the current time for this thread.

    Used for all subsequent logging on the same thread,
    until the next call to `set_time_sequence`.

    For instance: `set_time_sequence("frame_nr", frame_nr)`.

    You can remove a timeline again using `set_time_sequence("frame_nr", None)`.

    There is no requirement of monoticity. You can move the time backwards if you like.
    """
    rerun_sdk.set_time_sequence(timeline, sequence)


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
    rerun_sdk.set_time_seconds(timeline, seconds)


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
    rerun_sdk.set_time_nanos(timeline, nanos)


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
    rerun_sdk.log_text_entry(obj_path, text, level, color, timeless)


def log_scalar(
    obj_path: str,
    scalar: float,
    label: Optional[str] = None,
    color: Optional[Sequence[int]] = None,
    radius: Optional[float] = None,
    scattered: Optional[bool] = None,
) -> None:
    """
    Log a double-precision scalar that will be visualized as a timeseries plot.

    The current simulation time will be used for the time/X-axis, hence scalars cannot be
    timeless!

    See also examples/plots.

    ## Understanding the plot and attributes hierarchy

    Timeseries come in three parts: points, lines and finally the plots themselves.
    As a user of the Rerun SDK, your one and only entrypoint into that hierarchy is through the
    lowest-level layer: the points.

    When logging scalars and their attributes (label, color, radius, scattered) through this
    function, Rerun will turn them into points with similar attributes.
    From these points, lines with appropriate attributes can then be inferred; and from these
    inferred lines, plots with appropriate attributes will be inferred in turn!

    In terms of actual hierarchy:
    - Each space represents a single plot.
    - Each object path within a space that contains scalar data is a line within that plot.
    - Each logged scalar is a point.

    E.g. the following:
    ```
    rerun.log_scalar("trig/sin", sin(t), label="sin(t)", color=RED)
    rerun.log_scalar("trig/cos", cos(t), label="cos(t)", color=BLUE)
    ```
    will yield a single plot (space = `trig`), comprised of two lines (object paths `trig/sin`
    and `trig/cos`).

    ## Attributes

    The attributes you assigned (or not) to a scalar will affect all layers: points, lines and
    plots alike.

    ### `label`

    An optional label for the point.

    This won't show up on points at the moment, as our plots don't yet support displaying labels
    for individual points.

    If all points within a single object path (i.e. a line) share the same label, then this label
    will be used as the label for the line itself.
    Otherwise, the line will be named after the object path.

    The plot itself is named after the space it's in.

    ### `color`

    An optional color in the form of a RGB or RGBA triplet in 0-255 sRGB.
    If left unspecified, a pseudo-random color will be used instead. That same color will apply
    to all points residing in the same object path that don't have a color specified.

    Points within a single line do not have to share the same color, the line will have
    differently colored segments as appropriate.

    If all points within a single object path (i.e. a line) share the same color, then this color
    will be used as the line color in the plot legend.
    Otherwise, the line will appear grey in the legend.

    ### `radius`

    An optional radius for the point.

    Points within a single line do not have to share the same radius, the line will have
    differently sized segments as appropriate.

    If all points within a single object path (i.e. a line) share the same radius, then this radius
    will be used as the line width too.
    Otherwise, the line will use the default width of `1.0`.

    ### `scattered`

    Specifies whether the point should form a continuous line with its neighbours, or whether it
    should stand on its own, akin to a scatter plot.

    Points within a single line do not have to all share the same scatteredness: the line will
    switch between a scattered and a continous representation as required.
    """
    rerun_sdk.log_scalar(obj_path, scalar, label, color, radius, scattered)


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
    rect: Optional[npt.ArrayLike],
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
    * `label`: Optional text to show inside the rectangle.
    * `color`: Optional RGB or RGBA triplet in 0-255 sRGB.
    """
    rerun_sdk.log_rect(obj_path, rect_format.value, _to_sequence(rect), color, label, timeless)


def log_rects(
    obj_path: str,
    rects: Optional[npt.ArrayLike],
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
    * `labels`: Optional per-rectangle text to show inside the rectangle.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `colors`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
    * float32/float64: all color components should be in 0-1 linear space.

    """
    # Treat None the same as []
    if rects is None:
        rects = []
    rects = np.require(rects, dtype="float32")
    colors = _normalize_colors(colors)
    if labels is None:
        labels = []

    rerun_sdk.log_rects(obj_path, rect_format.value, rects, colors, labels, timeless)


def log_point(
    obj_path: str,
    position: Optional[npt.NDArray[np.float32]],
    *,
    color: Optional[Sequence[int]] = None,
    label: Optional[str] = None,
    class_id: Optional[int] = None,
    timeless: bool = False,
) -> None:
    """
    Log a 2D or 3D point, with optional color.

    Logging again to the same `obj_path` will replace the previous point.

    * `position`: 2x1 or 3x1 array
    * `color`: Optional color of the point
    * `label`: Optional text to show with the point
    * `class_id`: Optional class id for the point. The class id provides color and label if not specified explicitly.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `color`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
    * float32/float64: all color components should be in 0-1 linear space.
    """
    if position is not None:
        position = np.require(position, dtype="float32")
    rerun_sdk.log_point(obj_path, position, color, label, class_id, timeless)


def log_points(
    obj_path: str,
    positions: Optional[npt.NDArray[np.float32]],
    *,
    colors: Optional[Colors] = None,
    labels: Optional[Sequence[str]] = None,
    class_ids: Optional[ClassIds] = None,
    timeless: bool = False,
) -> None:
    """
    Log 2D or 3D points, with optional colors.

    Logging again to the same `obj_path` will replace all the previous points.

    * `positions`: Nx2 or Nx3 array
    * `color`: Optional colors of the points.
    * `labels`: Optional per-point text to show with the points
    * `class_id`: Optional class ids for the points. The class id provides colors and labels if not specified explicitly.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `colors`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
    * float32/float64: all color components should be in 0-1 linear space.

    """
    if positions is None:
        positions = np.require([], dtype="float32")
    else:
        positions = np.require(positions, dtype="float32")
    colors = _normalize_colors(colors)
    class_ids = _normalize_class_id(class_ids)
    if labels is None:
        labels = []

    rerun_sdk.log_points(obj_path, positions, colors, labels, class_ids, timeless)


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


def _normalize_class_id(class_ids: Optional[npt.ArrayLike] = None) -> npt.NDArray[np.uint16]:
    """Normalize flexible class id arrays."""
    if class_ids is None:
        return np.array((), dtype=np.uint16)
    else:
        class_ids_array = np.array(class_ids)
        if class_ids_array.dtype is np.uint8:
            class_ids_array = class_ids_array.astype(np.uint16)

        return np.require(class_ids_array, np.uint16)


# -----------------------------------------------------------------------------


def log_unknown_transform(obj_path: str, timeless: bool = False) -> None:
    """Log that this object is NOT in the same space as the parent, but you do not (yet) know how they relate."""
    rerun_sdk.log_unknown_transform(obj_path, timeless=timeless)


def log_rigid3(
    obj_path: str,
    *,
    parent_from_child: Optional[Tuple[npt.ArrayLike, npt.ArrayLike]] = None,
    child_from_parent: Optional[Tuple[npt.ArrayLike, npt.ArrayLike]] = None,
    xyz: str = "",
    timeless: bool = False,
) -> None:
    """
    Log a proper rigid 3D transform between this object and the parent.

    Set either `parent_from_child` or `child_from_parent` to
    a tuple of `(translation_xyz, quat_xyzw)`.

    `parent_from_child`
    -------------------
    `parent_from_child=(translation_xyz, quat_xyzw)`

    Also known as pose (e.g. camera extrinsics).

    The translation is the position of the object in the parent space.
    The resulting transform from child to parent corresponds to taking a point in the child space,
    rotating it by the given rotations, and then translating it by the given translation:

    `point_parent = translation + quat * point_child * quat*

    `child_from_parent`
    -------------------
    `child_from_parent=(translation_xyz, quat_xyzw)`

    the inverse of `parent_from_child`

    `xyz`
    ----
    Optionally set the view coordinates of this object, e.g. to `RDF` for `X=Right, Y=Down, Z=Forward`.
    This is a convenience for also calling `log_view_coordinates`.

    Example
    -------
    ```
    rerun.log_rigid3("world/camera", parent_from_child=(t,q))
    rerun.log_pinhole("world/camera/image", …)
    ```

    """
    if parent_from_child and child_from_parent:
        raise TypeError("Set either parent_from_child or child_from_parent, but not both")
    elif parent_from_child:
        (t, q) = parent_from_child
        rerun_sdk.log_rigid3(
            obj_path,
            parent_from_child=True,
            rotation_q=_to_sequence(q),
            translation=_to_sequence(t),
            timeless=timeless,
        )
    elif child_from_parent:
        (t, q) = child_from_parent
        rerun_sdk.log_rigid3(
            obj_path,
            parent_from_child=False,
            rotation_q=_to_sequence(q),
            translation=_to_sequence(t),
            timeless=timeless,
        )
    else:
        raise TypeError("Set either parent_from_child or child_from_parent")

    if xyz != "":
        log_view_coordinates(obj_path, xyz=xyz, timeless=timeless)


def log_pinhole(
    obj_path: str, *, child_from_parent: npt.ArrayLike, width: int, height: int, timeless: bool = False
) -> None:
    """
    Log a perspective camera model.

    This logs the pinhole model that projects points from the parent (camera) space to this space (image) such that:
    ```
    point_image_hom = child_from_parent * point_cam
    point_image = point_image_hom[:,1] / point_image_hom[2]
    ```

    Where `point_image_hom` is the projected point in the image space expressed in homogeneous coordinates.

    Example
    -------
    ```
    rerun.log_rigid3("world/camera", …)
    rerun.log_pinhole("world/camera/image", …)
    ```

    `child_from_parent`: Row-major intrinsics matrix for projecting from camera space to image space
    `resolution`: Array with [width, height] image resolution in pixels.

    """
    rerun_sdk.log_pinhole(
        obj_path,
        resolution=[width, height],
        child_from_parent=np.asarray(child_from_parent).T.tolist(),
        timeless=timeless,
    )


# -----------------------------------------------------------------------------


def log_view_coordinates(
    obj_path: str, *, xyz: str = "", up: str = "", right_handed: Optional[bool] = None, timeless: bool = False
) -> None:
    """
    Log the view coordinates for an object.

    Each object defines its own coordinate system, called a space.
    By logging view coordinates you can give semantic meaning to the XYZ axes of the space.
    This is for instance useful for camera objects ("what axis is forward?").

    For full control, set the `xyz` parameter to a three-letter acronym (`xyz="RDF"`). Each letter represents:

    * R: Right
    * L: Left
    * U: Up
    * D: Down
    * F: Forward
    * B: Back

    Some of the most common are:

    * "RDF": X=Right Y=Down Z=Forward  (right-handed)
    * "RUB"  X=Right Y=Up   Z=Back     (right-handed)
    * "RDB": X=Right Y=Down Z=Back     (left-handed)
    * "RUF": X=Right Y=Up   Z=Forward  (left-handed)

    Example
    -------
    ```
    rerun.log_view_coordinates("world/camera", xyz="RUB")
    ```

    For world-coordinates it's often conventient to just specify an up-axis.
    You can do so by using the `up`-parameter (where `up` is one of "+X", "-X", "+Y", "-Y", "+Z", "-Z"):

    ```
    rerun.log_view_coordinates("world", up="+Z", right_handed=True, timeless=True)
    rerun.log_view_coordinates("world", up="-Y", right_handed=False, timeless=True)
    ```

    """
    if xyz == "" and up == "":
        raise TypeError("You must set either 'xyz' or 'up'")
    if xyz != "" and up != "":
        raise TypeError("You must set either 'xyz' or 'up', but not both")
    if xyz != "":
        rerun_sdk.log_view_coordinates_xyz(obj_path, xyz, right_handed, timeless)
    else:
        if right_handed is None:
            right_handed = True
        rerun_sdk.log_view_coordinates_up_handedness(obj_path, up, right_handed, timeless)


# -----------------------------------------------------------------------------


def log_path(
    obj_path: str,
    positions: Optional[npt.NDArray[np.float32]],
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
    if positions is not None:
        positions = np.require(positions, dtype="float32")
    rerun_sdk.log_path(obj_path, positions, stroke_width, color, timeless)


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
    if positions is None:
        positions = []
    positions = np.require(positions, dtype="float32")
    rerun_sdk.log_line_segments(obj_path, positions, stroke_width, color, timeless)


def log_arrow(
    obj_path: str,
    origin: Optional[npt.ArrayLike],
    vector: Optional[npt.ArrayLike] = None,
    *,
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
    rerun_sdk.log_arrow(
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
    half_size: Optional[npt.ArrayLike],
    position: Optional[npt.ArrayLike] = None,
    rotation_q: Optional[npt.ArrayLike] = None,
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
    rerun_sdk.log_obb(
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
    *,
    timeless: bool = False,
) -> None:
    """
    Log an image made up of uint8 or uint16 class-ids.

    The image should have 1 channels.

    Supported `dtype`s:
    * uint8: components should be 0-255 class ids
    * uint16: components should be 0-65535 class ids

    """
    # Catch some errors early:
    if len(image.shape) < 2 or 3 < len(image.shape):
        raise TypeError(f"Expected image, got array of shape {image.shape}")

    if len(image.shape) == 3:
        depth = image.shape[2]
        if depth != 1:
            raise TypeError(f"Expected image depth of 1. Instead got array of shape {image.shape}")

    if not isinstance(image, np.ndarray):
        image = np.array(image)
    if image.dtype == "uint8":
        rerun_sdk.log_tensor_u8(obj_path, image, None, None, rerun_sdk.TensorDataMeaning.ClassId, timeless)
    elif image.dtype == "uint16":
        rerun_sdk.log_tensor_u16(obj_path, image, None, None, rerun_sdk.TensorDataMeaning.ClassId, timeless)
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
        rerun_sdk.log_tensor_u8(obj_path, tensor, names, meter, None, timeless)
    elif tensor.dtype == "uint16":
        rerun_sdk.log_tensor_u16(obj_path, tensor, names, meter, None, timeless)
    elif tensor.dtype == "float32":
        rerun_sdk.log_tensor_f32(obj_path, tensor, names, meter, None, timeless)
    elif tensor.dtype == "float64":
        rerun_sdk.log_tensor_f32(obj_path, tensor.astype("float32"), names, meter, None, timeless)
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

    rerun_sdk.log_mesh_file(obj_path, mesh_format.value, mesh_file, transform, timeless)


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
    rerun_sdk.log_image_file(obj_path, img_path, img_format, timeless)


def _to_sequence(array: Optional[npt.ArrayLike]) -> Optional[Sequence[float]]:
    if isinstance(array, np.ndarray):
        return np.require(array, float).tolist()  # type: ignore[no-any-return]

    return array  # type: ignore[return-value]


def log_cleared(obj_path: str, *, recursive: bool = False) -> None:
    """
    Indicate that an object at a given path should no longer be displayed.

    If `recursive` is True this will also clear all sub-paths
    """
    rerun_sdk.log_cleared(obj_path, recursive)


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


def log_annotation_context(
    obj_path: str,
    class_descriptions: Sequence[ClassDescriptionLike],
    *,
    timeless: bool = True,
) -> None:
    """
    Log an annotation context made up of a collection of ClassDescriptions.

    Any object needing to access the annotation context will find it by searching the
    path upward. If all objects share the same you can simply log it to the
    root ("/"), or if you want a per-object ClassDescriptions log it to the same path as
    your object.

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

    rerun_sdk.log_annotation_context(obj_path, tuple_class_descriptions, timeless)


def set_visible(obj_path: str, visibile: bool) -> None:
    """
    set_visible has been deprecated.

    The replacement is `log_cleared()`.
    See: https://github.com/rerun-io/rerun/pull/285 for details
    """
    # This is a slight abose of DeprecationWarning compared to using
    # warning.warn, but there is no function to call here anymore.
    # this is (slightly) better than just failing on an undefined function
    # TODO(jleibs) Remove after 11/25
    raise DeprecationWarning("set_visible has been deprecated. please use log_cleared")
