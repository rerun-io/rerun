from enum import Enum
from typing import Optional, Sequence, Union

import numpy as np
import numpy.typing as npt
from rerun.log import (
    Color,
    Colors,
    OptionalClassIds,
    _normalize_colors,
    _normalize_ids,
    _normalize_labels,
)
from rerun.log.error_utils import _send_warning

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
    entity_path: str,
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
    from rerun.components.annotation import ClassIdArray
    from rerun.components.color import ColorRGBAArray
    from rerun.components.label import LabelArray
    from rerun.components.rect2d import Rect2DArray

    if np.any(rect):  # type: ignore[arg-type]
        rects = np.asarray([rect], dtype="float32")
    else:
        rects = np.zeros((0, 4), dtype="float32")
    assert type(rects) is np.ndarray

    comps = {"rerun.rect2d": Rect2DArray.from_numpy_and_format(rects, rect_format)}

    if color:
        colors = _normalize_colors([color])
        comps["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if label:
        comps["rerun.label"] = LabelArray.new([label])

    if class_id:
        class_ids = _normalize_ids([class_id])
        comps["rerun.class_id"] = ClassIdArray.from_numpy(class_ids)

    bindings.log_arrow_msg(entity_path, components=comps, timeless=timeless)


def log_rects(
    entity_path: str,
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

    Logging again to the same `entity_path` will replace all the previous rectangles.

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
    if np.any(rects):  # type: ignore[arg-type]
        rects = np.asarray(rects, dtype="float32")
    else:
        rects = np.zeros((0, 4), dtype="float32")
    assert type(rects) is np.ndarray

    colors = _normalize_colors(colors)
    class_ids = _normalize_ids(class_ids)
    labels = _normalize_labels(labels)

    from rerun.components.annotation import ClassIdArray
    from rerun.components.color import ColorRGBAArray
    from rerun.components.instance import InstanceArray
    from rerun.components.label import LabelArray
    from rerun.components.rect2d import Rect2DArray

    identifiers_np = np.array((), dtype="int64")
    if identifiers:
        try:
            identifiers = [int(id) for id in identifiers]
            identifiers_np = np.array(identifiers, dtype="int64")
        except ValueError:
            _send_warning("Only integer identifies supported", 1)

    # 0 = instanced, 1 = splat
    comps = [{}, {}]  # type: ignore[var-annotated]
    comps[0]["rerun.rect2d"] = Rect2DArray.from_numpy_and_format(rects, rect_format)

    if len(identifiers_np):
        comps[0]["rerun.instance"] = InstanceArray.from_numpy(identifiers_np)

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

    bindings.log_arrow_msg(entity_path, components=comps[0], timeless=timeless)

    if comps[1]:
        comps[1]["rerun.instance"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=comps[1], timeless=timeless)
