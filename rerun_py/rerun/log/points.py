from typing import Optional, Sequence, Union

import numpy as np
import numpy.typing as npt
from rerun.components.radius import RadiusArray
from rerun.log import (
    EXP_ARROW,
    Color,
    Colors,
    OptionalClassIds,
    OptionalKeyPointIds,
    _normalize_colors,
    _normalize_ids,
    _normalize_radii,
)

from rerun import bindings

__all__ = [
    "log_point",
    "log_points",
]


def log_point(
    obj_path: str,
    position: Union[Sequence[float], npt.NDArray[np.float32], None],
    *,
    radius: Optional[float] = None,
    color: Optional[Sequence[int]] = None,
    label: Optional[str] = None,
    class_id: Optional[int] = None,
    keypoint_id: Optional[int] = None,
    timeless: bool = False,
) -> None:
    """
    Log a 2D or 3D point, with optional color.

    Logging again to the same `obj_path` will replace the previous point.

    * `position`: 2x1 or 3x1 array
    * `radius`: Optional radius (make it a sphere)
    * `color`: Optional color of the point
    * `label`: Optional text to show with the point
    * `class_id`: Optional class id for the point.
        The class id provides color and label if not specified explicitly.
    * `keypoint_id`: Optional key point id for the point, identifying it within a class.
        If keypoint_id is passed but no class_id was specified, class_id will be set to 0.
        This is useful to identify points within a single classification (which is identified with class_id).
        E.g. the classification might be 'Person' and the keypoints refer to joints on a detected skeleton.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `color`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
    * float32/float64: all color components should be in 0-1 linear space.
    """
    if keypoint_id is not None and class_id is None:
        class_id = 0
    if position is not None:
        position = np.require(position, dtype="float32")

    if EXP_ARROW.classic_log_gate():
        bindings.log_point(
            obj_path=obj_path,
            position=position,
            radius=radius,
            color=color,
            label=label,
            class_id=class_id,
            keypoint_id=keypoint_id,
            timeless=timeless,
        )

    if EXP_ARROW.arrow_log_gate():
        from rerun.components.color import ColorRGBAArray
        from rerun.components.label import LabelArray
        from rerun.components.point import Point2DArray, Point3DArray

        comps = {}

        if position is not None:
            if position.shape[0] == 2:
                comps["rerun.point2d"] = Point2DArray.from_numpy(position.reshape(1, 2))
            elif position.shape[0] == 3:
                comps["rerun.point3d"] = Point3DArray.from_numpy(position.reshape(1, 3))
            else:
                raise TypeError("Positions should be either 1x2 or 1x3")

        if color:
            colors = _normalize_colors([color])
            comps["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

        if label:
            comps["rerun.label"] = LabelArray.new([label])

        bindings.log_arrow_msg(f"arrow/{obj_path}", components=comps)


def log_points(
    obj_path: str,
    positions: Optional[npt.NDArray[np.float32]],
    *,
    identifiers: Optional[Sequence[Union[str, int]]] = None,
    colors: Optional[Union[Color, Colors]] = None,
    radii: Optional[npt.ArrayLike] = None,
    labels: Optional[Sequence[str]] = None,
    class_ids: OptionalClassIds = None,
    keypoint_ids: OptionalKeyPointIds = None,
    timeless: bool = False,
) -> None:
    """
    Log 2D or 3D points, with optional colors.

    Logging again to the same `obj_path` will replace all the previous points.

    * `positions`: Nx2 or Nx3 array
    * `identifiers`: per-point identifiers - unique names or numbers that show up when you hover the points.
      In the future these will be used to track the points over time.
    * `color`: Optional colors of the points.
    * `labels`: Optional per-point text to show with the points
    * `class_ids`: Optional class ids for the points.
        The class id provides colors and labels if not specified explicitly.
    * `keypoint_ids`: Optional key point ids for the points, identifying them within a class.
        If keypoint_ids are passed in but no class_ids were specified, class_id will be set to 0.
        This is useful to identify points within a single classification (which is identified with class_id).
        E.g. the classification might be 'Person' and the keypoints refer to joints on a detected skeleton.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported `dtype`s for `colors`:
    * uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
    * float32/float64: all color components should be in 0-1 linear space.

    """
    if keypoint_ids is not None and class_ids is None:
        class_ids = 0
    if positions is None:
        positions = np.require([], dtype="float32")
    else:
        positions = np.require(positions, dtype="float32")

    identifiers = [] if identifiers is None else [str(s) for s in identifiers]

    colors = _normalize_colors(colors)
    class_ids = _normalize_ids(class_ids)
    keypoint_ids = _normalize_ids(keypoint_ids)
    radii = _normalize_radii(radii)
    if labels is None:
        labels = []

    if EXP_ARROW.classic_log_gate():
        bindings.log_points(
            obj_path=obj_path,
            positions=positions,
            identifiers=identifiers,
            colors=colors,
            radii=radii,
            labels=labels,
            class_ids=class_ids,
            keypoint_ids=keypoint_ids,
            timeless=timeless,
        )

    if EXP_ARROW.arrow_log_gate():
        from rerun.components.color import ColorRGBAArray
        from rerun.components.label import LabelArray
        from rerun.components.point import Point2DArray, Point3DArray

        comps = {}

        if positions.any():
            if positions.shape[1] == 2:
                comps["rerun.point2d"] = Point2DArray.from_numpy(positions)
            elif positions.shape[1] == 3:
                comps["rerun.point3d"] = Point3DArray.from_numpy(positions)
            else:
                raise TypeError("Positions should be either Nx2 or Nx3")

        if len(colors):
            comps["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

        if len(radii):
            comps["rerun.radius"] = RadiusArray.from_numpy(radii)

        if labels:
            comps["rerun.label"] = LabelArray.new(labels)

        bindings.log_arrow_msg(f"arrow/{obj_path}", components=comps)
