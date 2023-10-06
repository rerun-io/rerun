from __future__ import annotations

from enum import Enum
from typing import Any, Sequence

import numpy as np
import numpy.typing as npt
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.any_value import AnyValues
from rerun.archetypes import Boxes2D
from rerun.error_utils import _send_warning_or_raise
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


@deprecated(
    """Please migrate to `rr.log(…, rr.Boxes2D(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
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

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Boxes2D][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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
    if rect is None:
        rect = []

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


@deprecated(
    """Please migrate to `rr.log(…, rr.Boxes2D(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
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

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Boxes2D][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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
    from rerun import Box2DFormat

    if rects is None:
        raise ValueError("`rects` argument must be set")

    if np.any(rects):
        rects = np.asarray(rects, dtype="float32")
        if rects.ndim == 1:
            rects = np.expand_dims(rects, axis=0)
    else:
        rects = np.zeros((0, 4), dtype="float32")
    assert type(rects) is np.ndarray

    recording = RecordingStream.to_native(recording)

    identifiers_np = None
    if identifiers is not None:
        try:
            identifiers_np = np.require(identifiers, dtype="uint64")
        except ValueError:
            _send_warning_or_raise("Only integer identifiers supported", 1)

    box2d_format = Box2DFormat.XYWH
    if rect_format == RectFormat.XYWH:
        box2d_format = Box2DFormat.XYWH
    elif rect_format == RectFormat.YXHW:
        box2d_format = Box2DFormat.YXHW
    elif rect_format == RectFormat.XYXY:
        box2d_format = Box2DFormat.XYXY
    elif rect_format == RectFormat.YXYX:
        box2d_format = Box2DFormat.YXYX
    elif rect_format == RectFormat.XCYCWH:
        box2d_format = Box2DFormat.XCYCWH
    elif rect_format == RectFormat.XCYCW2H2:
        box2d_format = Box2DFormat.XCYCW2H2
    else:
        box2d_format = Box2DFormat.XYWH

    arch = Boxes2D(
        array=rects,
        array_format=box2d_format,
        colors=colors,
        draw_order=draw_order,
        labels=labels,
        class_ids=class_ids,
        instance_keys=identifiers_np,
    )
    return log(entity_path, arch, AnyValues(**(ext or {})), timeless=timeless, recording=recording)
