from __future__ import annotations

from enum import Enum

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import (
    REGISTERED_COMPONENT_NAMES,
    ComponentTypeFactory,
    build_dense_union,
)

__all__ = [
    "Rect2DArray",
    "Rect2DType",
    "RectFormat",
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


class Rect2DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy_and_format(array: npt.NDArray[np.float_], rect_format: RectFormat) -> Rect2DArray:
        """Build a `Rect2DArray` from an Nx4 numpy array."""
        # Inner is a FixedSizeList<4>
        values = pa.array(array.flatten(), type=pa.float32())
        inner = pa.FixedSizeListArray.from_arrays(values=values, type=Rect2DType.storage_type[0].type)
        storage = build_dense_union(data_type=Rect2DType.storage_type, discriminant=rect_format.value, child=inner)
        storage.validate(full=True)
        # TODO(john) enable extension type wrapper
        # return cast(Rect2DArray, pa.ExtensionArray.from_storage(Rect2DType(), storage))
        return storage  # type: ignore[no-any-return]


Rect2DType = ComponentTypeFactory("Rect2DType", Rect2DArray, REGISTERED_COMPONENT_NAMES["rerun.rect2d"])

pa.register_extension_type(Rect2DType())
