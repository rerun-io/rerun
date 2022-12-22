import logging
from enum import Enum
from typing import Optional, Sequence, Union

import numpy as np
import numpy.typing as npt
from rerun.log import (
    EXP_ARROW,
    Color,
    Colors,
    OptionalClassIds,
    _normalize_colors,
    _normalize_ids,
    _to_sequence,
)

from rerun import bindings

__all__ = [
    "RectFormat",
    "log_rect",
    "log_rects",
]


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
    class_id: Optional[int] = None,
    timeless: bool = False,
) -> None:
    """
    Log a 2D rectangle.

    * `rect`: the recangle in [x, y, w, h], or some format you pick with the
    `rect_format` argument.
    * `rect_format`: how to interpret the `rect` argument
    * `color`: Optional RGB or RGBA triplet in 0-255 sRGB.
    * `label`: Optional text to show inside the rectangle.
    * `class_id`: Optional class id for the rectangle.
       The class id provides color and label if not specified explicitly.
    """
    if EXP_ARROW.classic_log_gate():
        bindings.log_rect(obj_path, rect_format.value, _to_sequence(rect), color, label, class_id, timeless)

    if EXP_ARROW.arrow_log_gate():
        logging.warning("log_rect() not yet implemented for Arrow.")


def log_rects(
    obj_path: str,
    rects: Optional[npt.ArrayLike],
    *,
    rect_format: RectFormat = RectFormat.XYWH,
    identifiers: Optional[Sequence[Union[str, int]]] = None,
    colors: Optional[Union[Color, Colors]] = None,
    labels: Optional[Sequence[str]] = None,
    class_ids: OptionalClassIds = None,
    timeless: bool = False,
) -> None:
    """
    Log multiple 2D rectangles.

    Logging again to the same `obj_path` will replace all the previous rectangles.

    * `rects`: Nx4 numpy array, where each row is [x, y, w, h], or some format you pick with the `rect_format`
    argument.
    * `rect_format`: how to interpret the `rect` argument
    * `identifiers`: per-point identifiers - unique names or numbers that show up when you hover the rectangles.
      In the future these will be used to track the rectangles over time.
    * `labels`: Optional per-rectangle text to show inside the rectangle.
    * `class_ids`: Optional class ids for the rectangles.
      The class id provides colors and labels if not specified explicitly.

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
    rects = np.asarray(rects, dtype="float32")
    if len(rects) == 0:
        rects = rects.reshape((0, 4))

    identifiers = [] if identifiers is None else [str(s) for s in identifiers]
    colors = _normalize_colors(colors)
    class_ids = _normalize_ids(class_ids)
    if labels is None:
        labels = []

    if EXP_ARROW.classic_log_gate():
        bindings.log_rects(
            obj_path=obj_path,
            rect_format=rect_format.value,
            identifiers=identifiers,
            rects=rects,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            timeless=timeless,
        )

    if EXP_ARROW.arrow_log_gate():
        from rerun.components.color import ColorRGBAArray
        from rerun.components.label import LabelArray
        from rerun.components.rect2d import Rect2DArray

        comps = {"rerun.rect2d": Rect2DArray.from_numpy_and_format(rects, rect_format)}

        if len(colors):
            comps["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

        if labels:
            comps["rerun.label"] = LabelArray.new(labels)

        bindings.log_arrow_msg(f"arrow/{obj_path}", components=comps)
