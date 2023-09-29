from __future__ import annotations

from enum import Enum
from typing import Any

import numpy as np
import numpy.typing as npt

from rerun.error_utils import _send_warning

from .. import components, datatypes


class Box2DFormat(Enum):
    """How to specify 2D boxes (axis-aligned bounding boxes)."""

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


class Boxes2DExt:
    def __init__(
        self: Any,
        *,
        sizes: datatypes.Vec2DArrayLike | None = None,
        mins: datatypes.Vec2DArrayLike | None = None,
        half_sizes: datatypes.Vec2DArrayLike | None = None,
        centers: datatypes.Vec2DArrayLike | None = None,
        array: npt.ArrayLike | None = None,
        array_format: Box2DFormat = Box2DFormat.XYWH,
        radii: components.RadiusArrayLike | None = None,
        colors: datatypes.ColorArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        draw_order: components.DrawOrderLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        instance_keys: components.InstanceKeyArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Boxes2D archetype.

        Parameters
        ----------
        sizes:
            Full extents in x/y. Specify this instead of `half_sizes`
        half_sizes:
            All half-extents that make up the batch of boxes. Specify this instead of `sizes`
        mins:
            Minimum coordinates of the boxes. Specify this instead of `centers`.

            Only valid when used together with either `sizes` or `half_sizes`.
        array:
            An array of boxes in the format specified by `array_format`.
        array_format:
            How to interpret the data in `array`.
        centers:
            Optional center positions of the boxes.
        colors:
            Optional colors for the boxes.
        radii:
            Optional radii for the lines that make up the boxes.
        labels:
            Optional text labels for the boxes.
        draw_order:
            An optional floating point value that specifies the 2D drawing order.
            Objects with higher values are drawn on top of those with lower values.

            The default for 2D boxes is 10.0.
        class_ids:
            Optional `ClassId`s for the boxes.

            The class ID provides colors and labels if not specified explicitly.
        instance_keys:
            Unique identifiers for each individual boxes in the batch.
        """
        if array is not None:
            if half_sizes is not None:
                _send_warning("Cannot specify both `array` and `half_sizes` at the same time.", 1)
            if sizes is not None:
                _send_warning("Cannot specify both `array` and `sizes` at the same time.", 1)
            if mins is not None:
                _send_warning("Cannot specify both `array` and `mins` at the same time.", 1)
            if centers is not None:
                _send_warning("Cannot specify both `array` and `centers` at the same time.", 1)

            if np.any(array):
                array = np.asarray(array, dtype="float32")
                if array.ndim == 1:
                    array = np.expand_dims(array, axis=0)
            else:
                array = np.zeros((0, 4), dtype="float32")
            assert type(array) is np.ndarray

            if array_format == Box2DFormat.XYWH:
                half_sizes = array[:, 2:4] / 2
                centers = array[:, 0:2] + half_sizes
            elif array_format == Box2DFormat.YXHW:
                half_sizes = np.flip(array[:, 2:4]) / 2
                centers = np.flip(array[:, 0:2]) + half_sizes
            elif array_format == Box2DFormat.XYXY:
                min = array[:, 0:2]
                max = array[:, 2:4]
                centers = (min + max) / 2
                half_sizes = max - centers
            elif array_format == Box2DFormat.YXYX:
                min = np.flip(array[:, 0:2])
                max = np.flip(array[:, 2:4])
                centers = (min + max) / 2
                half_sizes = max - centers
            elif array_format == Box2DFormat.XCYCWH:
                half_sizes = array[:, 2:4] / 2
                centers = array[:, 0:2]
            elif array_format == Box2DFormat.XCYCW2H2:
                half_sizes = array[:, 2:4]
                centers = array[:, 0:2]
            else:
                raise ValueError(f"Unknown Box2D format {array_format}")
        else:
            if sizes is not None:
                if half_sizes is not None:
                    _send_warning("Cannot specify both `sizes` and `half_sizes` at the same time.", 1)

                sizes = np.asarray(sizes, dtype=np.float32)
                half_sizes = sizes / 2.0

            if mins is not None:
                if centers is not None:
                    _send_warning("Cannot specify both `mins` and `centers` at the same time.", 1)

                # already converted `sizes` to `half_sizes`
                if half_sizes is None:
                    _send_warning("Cannot specify `mins` without `sizes` or `half_sizes`.", 1)
                    half_sizes = np.asarray([1, 1], dtype=np.float32)

                mins = np.asarray(mins, dtype=np.float32)
                half_sizes = np.asarray(half_sizes, dtype=np.float32)
                centers = mins + half_sizes

        print(half_sizes, sizes, centers, mins)

        self.__attrs_init__(
            half_sizes=half_sizes,
            centers=centers,
            radii=radii,
            colors=colors,
            labels=labels,
            draw_order=draw_order,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
