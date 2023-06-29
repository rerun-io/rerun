from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import numpy.typing as npt

from rerun import bindings
from rerun.components.annotation import ClassIdArray
from rerun.components.color import ColorRGBAArray
from rerun.components.draw_order import DrawOrderArray
from rerun.components.instance import InstanceArray
from rerun.components.label import LabelArray
from rerun.components.rect2d import Rect2DArray, RectFormat
from rerun.log import Color, Colors, OptionalClassIds, _normalize_colors, _normalize_ids, _normalize_labels
from rerun.log.error_utils import _send_warning
from rerun.log.extension_components import _add_extension_components
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "RectFormat",
    "log_rect",
    "log_rects",
]


@log_decorator
def log_rect(
    entity_path: str,
    rect: npt.ArrayLike | None,
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
    recording = RecordingStream.to_native(recording)

    if np.any(rect):  # type: ignore[arg-type]
        rects = np.asarray([rect], dtype="float32")
    else:
        rects = np.zeros((0, 4), dtype="float32")
    assert type(rects) is np.ndarray

    instanced: dict[str, Any] = {}
    splats: dict[str, Any] = {}

    instanced["rerun.rect2d"] = Rect2DArray.from_numpy_and_format(rects, rect_format)

    if color is not None:
        colors = _normalize_colors(color)
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if label:
        instanced["rerun.label"] = LabelArray.new([label])

    if class_id:
        class_ids = _normalize_ids([class_id])
        instanced["rerun.class_id"] = ClassIdArray.from_numpy(class_ids)

    if draw_order is not None:
        instanced["rerun.draw_order"] = DrawOrderArray.splat(draw_order)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(
            entity_path,
            components=splats,
            timeless=timeless,
            recording=recording,
        )

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(
            entity_path,
            components=instanced,
            timeless=timeless,
            recording=recording,
        )


@log_decorator
def log_rects(
    entity_path: str,
    rects: npt.ArrayLike | None,
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
    recording = RecordingStream.to_native(recording)

    # Treat None the same as []
    if np.any(rects):  # type: ignore[arg-type]
        rects = np.asarray(rects, dtype="float32")
    else:
        rects = np.zeros((0, 4), dtype="float32")
    assert type(rects) is np.ndarray

    colors = _normalize_colors(colors)
    class_ids = _normalize_ids(class_ids)
    labels = _normalize_labels(labels)

    identifiers_np = np.array((), dtype="int64")
    if identifiers:
        try:
            identifiers = [int(id) for id in identifiers]
            identifiers_np = np.array(identifiers, dtype="int64")
        except ValueError:
            _send_warning("Only integer identifiers supported", 1)

    # 0 = instanced, 1 = splat
    comps = [{}, {}]  # type: ignore[var-annotated]
    comps[0]["rerun.rect2d"] = Rect2DArray.from_numpy_and_format(rects, rect_format)

    if len(identifiers_np):
        comps[0]["rerun.instance_key"] = InstanceArray.from_numpy(identifiers_np)

    if len(colors):
        is_splat = len(colors.shape) == 1
        if is_splat:
            colors = colors.reshape(1, len(colors))
        comps[is_splat]["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if len(labels):
        is_splat = len(labels) == 1
        comps[is_splat]["rerun.label"] = LabelArray.new(labels)

    if len(class_ids):
        is_splat = len(class_ids) == 1
        comps[is_splat]["rerun.class_id"] = ClassIdArray.from_numpy(class_ids)

    if draw_order is not None:
        comps[True]["rerun.draw_order"] = DrawOrderArray.splat(draw_order)

    if ext:
        _add_extension_components(comps[0], comps[1], ext, identifiers_np)

    if comps[1]:
        comps[1]["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(
            entity_path,
            components=comps[1],
            timeless=timeless,
            recording=recording,
        )

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    bindings.log_arrow_msg(
        entity_path,
        components=comps[0],
        timeless=timeless,
        recording=recording,
    )
