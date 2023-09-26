from __future__ import annotations

from enum import Enum
from typing import Any, Sequence

import numpy as np
import numpy.typing as npt

from rerun._log import log
from rerun.archetypes import Boxes2D
from rerun.error_utils import _send_warning
from rerun.log_deprecated import Color, Colors, OptionalClassIds
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "RectFormat",
    "log_rect",
    "log_rects",
]


class RectFormat(Enum):
    """How to specify rectangles (axis-aligned bounding boxes)."""

    XYWH = "XYWH"
    """[x,y,w,h], with x,y = left,top."""

    YXHW = "YXHW"
    """[y,x,h,w], with x,y = left,top."""

    XYXY = "XYXY"
    """[x0, y0, x1, y1], with x0,y0 = left,top and x1,y1 = right,bottom."""

    YXYX = "YXYX"
    """[y0, x0, y1, x1], with x0,y0 = left,top and x1,y1 = right,bottom."""

    XCYCWH = "XCYCWH"
    """[x_center, y_center, width, height]."""

    XCYCW2H2 = "XCYCW2H2"
    """[x_center, y_center, width/2, height/2]."""


@log_decorator
def log_rect(
    entity_path: str,
    rect: npt.ArrayLike,
    *,
    rect_format: RectFormat = RectFormat.XYWH,
    color: Color | None = None,
    label: str | None = None,
    class_id: int | None = None,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a 2D rectangle.

    Parameters
    ----------
    entity_path:
        Path to the rectangle in the space hierarchy.
    rect:
        the rectangle in [x, y, w, h], or some format you pick with the `rect_format` argument.
    rect_format:
        how to interpret the `rect` argument
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    label:
        Optional text to show inside the rectangle.
    class_id:
        Optional class id for the rectangle.
        The class id provides color and label if not specified explicitly.
        See [rerun.log_annotation_context][]
    draw_order:
        An optional floating point value that specifies the 2D drawing order.
        Objects with higher values are drawn on top of those with lower values.
        The default for rects is 10.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
         If true, the rect will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    log_rects(
        entity_path,
        rects=rect,
        rect_format=rect_format,
        colors=color,
        labels=[label] if label is not None else None,
        class_ids=[class_id] if class_id is not None else None,
        draw_order=draw_order,
        ext=ext,
        timeless=timeless,
        recording=recording,
    )


@log_decorator
def log_rects(
    entity_path: str,
    rects: npt.ArrayLike,
    *,
    rect_format: RectFormat = RectFormat.XYWH,
    identifiers: Sequence[int] | None = None,
    colors: Color | Colors | None = None,
    labels: Sequence[str] | None = None,
    class_ids: OptionalClassIds = None,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log multiple 2D rectangles.

    Logging again to the same `entity_path` will replace all the previous rectangles.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `colors`:
    --------------------------------
     - uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
     - float32/float64: all color components should be in 0-1 linear space.

    Parameters
    ----------
    entity_path:
        Path to the rectangles in the space hierarchy.
    rects:
        Nx4 numpy array, where each row is [x, y, w, h], or some format you pick with the `rect_format` argument.
    rect_format:
        how to interpret the `rect` argument
    identifiers:
        Unique numeric id that shows up when you hover or select the point.
    colors:
        Optional per-rectangle gamma-space RGB or RGBA as 0-1 floats or 0-255 integers.
    labels:
        Optional per-rectangle text to show inside the rectangle.
    class_ids:
        Optional class ids for the rectangles.
        The class id provides colors and labels if not specified explicitly.
        See [rerun.log_annotation_context][]
    draw_order:
        An optional floating point value that specifies the 2D drawing order.
        Objects with higher values are drawn on top of those with lower values.
        The default for rects is 10.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
            If true, the rects will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    if rects is None:
        raise ValueError("`rects` argument must be set")

    if np.any(rects):
        rects = np.asarray(rects, dtype="float32")
        if rects.ndim == 1:
            rects = np.expand_dims(rects, axis=0)
    else:
        rects = np.zeros((0, 4), dtype="float32")
    assert type(rects) is np.ndarray

    if rect_format == RectFormat.XYWH:
        half_sizes = rects[:, 2:4] / 2
        centers = rects[:, 0:2] + half_sizes
    elif rect_format == RectFormat.YXHW:
        half_sizes = rects[:, 4:2] / 2
        centers = rects[:, 2:0] + half_sizes
    elif rect_format == RectFormat.XYXY:
        min = rects[:, 0:2]
        max = rects[:, 2:4]
        centers = (min + max) / 2
        half_sizes = max - centers
    elif rect_format == RectFormat.YXYX:
        min = rects[:, 2:0]
        max = rects[:, 4:2]
        centers = (min + max) / 2
        half_sizes = max - centers
    elif rect_format == RectFormat.XCYCWH:
        half_sizes = rects[:, 2:4] / 2
        centers = rects[:, 0:2]
    elif rect_format == RectFormat.XCYCW2H2:
        half_sizes = rects[:, 2:4]
        centers = rects[:, 0:2]
    else:
        raise ValueError(f"Unknown rect format {rect_format}")

    recording = RecordingStream.to_native(recording)

    identifiers_np = None
    if identifiers is not None:
        try:
            identifiers_np = np.require(identifiers, dtype="uint64")
        except ValueError:
            _send_warning("Only integer identifiers supported", 1)

    arch = Boxes2D(
        half_sizes=half_sizes,
        centers=centers,
        colors=colors,
        draw_order=draw_order,
        labels=labels,
        class_ids=class_ids,
        instance_keys=identifiers_np,
    )
    return log(entity_path, arch, ext=ext, timeless=timeless, recording=recording)
