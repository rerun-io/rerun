from typing import Any, Dict, Optional, Sequence, Union

import numpy as np
import numpy.typing as npt

from depthai_viewer import bindings
from depthai_viewer.components.annotation import ClassIdArray
from depthai_viewer.components.color import ColorRGBAArray
from depthai_viewer.components.instance import InstanceArray
from depthai_viewer.components.label import LabelArray
from depthai_viewer.components.point import Point2DArray, Point3DArray
from depthai_viewer.components.radius import RadiusArray
from depthai_viewer.log import (
    Color,
    Colors,
    OptionalClassIds,
    OptionalKeyPointIds,
    _normalize_colors,
    _normalize_ids,
    _normalize_labels,
    _normalize_radii,
)
from depthai_viewer.log.error_utils import _send_warning
from depthai_viewer.log.extension_components import _add_extension_components
from depthai_viewer.log.log_decorator import log_decorator

__all__ = [
    "log_point",
    "log_points",
]


@log_decorator
def log_point(
    entity_path: str,
    position: Optional[npt.ArrayLike] = None,
    *,
    radius: Optional[float] = None,
    color: Optional[Color] = None,
    label: Optional[str] = None,
    class_id: Optional[int] = None,
    keypoint_id: Optional[int] = None,
    ext: Optional[Dict[str, Any]] = None,
    timeless: bool = False,
) -> None:
    """
    Log a 2D or 3D point, with a position and optional color, radii, label, etc.

    Logging again to the same `entity_path` will replace the previous point.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA represented as a 2-element or 3-element sequence.

    Supported dtypes for `color`:
    -----------------------------
     - uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
     - float32/float64: all color components should be in 0-1 linear space.

    Parameters
    ----------
    entity_path:
        Path to the point in the space hierarchy.
    position:
        Any 2-element or 3-element array-like.
    radius:
        Optional radius (make it a sphere).
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    label:
        Optional text to show with the point.
    class_id:
        Optional class id for the point.
        The class id provides color and label if not specified explicitly.
        See [rerun.log_annotation_context][]
    keypoint_id:
        Optional key point id for the point, identifying it within a class.
        If keypoint_id is passed but no class_id was specified, class_id will be set to 0.
        This is useful to identify points within a single classification (which is identified with class_id).
        E.g. the classification might be 'Person' and the keypoints refer to joints on a detected skeleton.
        See [rerun.log_annotation_context][]
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the point will be timeless (default: False).

    """

    if keypoint_id is not None and class_id is None:
        class_id = 0
    if position is not None:
        position = np.require(position, dtype="float32")

    instanced: Dict[str, Any] = {}
    splats: Dict[str, Any] = {}

    if position is not None:
        if position.size == 2:
            instanced["rerun.point2d"] = Point2DArray.from_numpy(position.reshape(1, 2))
        elif position.size == 3:
            instanced["rerun.point3d"] = Point3DArray.from_numpy(position.reshape(1, 3))
        else:
            raise TypeError("Position must have a total size of 2 or 3")

    if color:
        colors = _normalize_colors([color])
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if radius:
        radii = _normalize_radii([radius])
        instanced["rerun.radius"] = RadiusArray.from_numpy(radii)

    if label:
        instanced["rerun.label"] = LabelArray.new([label])

    if class_id:
        class_ids = _normalize_ids([class_id])
        instanced["rerun.class_id"] = ClassIdArray.from_numpy(class_ids)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless)


@log_decorator
def log_points(
    entity_path: str,
    positions: Optional[npt.ArrayLike] = None,
    *,
    identifiers: Optional[npt.ArrayLike] = None,
    colors: Optional[Union[Color, Colors]] = None,
    radii: Optional[npt.ArrayLike] = None,
    labels: Optional[Sequence[str]] = None,
    class_ids: OptionalClassIds = None,
    keypoint_ids: OptionalKeyPointIds = None,
    ext: Optional[Dict[str, Any]] = None,
    timeless: bool = False,
) -> None:
    """
    Log 2D or 3D points, with positions and optional colors, radii, labels, etc.

    Logging again to the same `entity_path` will replace all the previous points.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported dtypes for `colors`:
    ------------------------------
     - uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
     - float32/float64: all color components should be in 0-1 linear space.


    Parameters
    ----------
    entity_path:
        Path to the points in the space hierarchy.
    positions:
        Nx2 or Nx3 array
    identifiers:
        Unique numeric id that shows up when you hover or select the point.
    colors:
        Optional colors of the points.
        The colors are interpreted as RGB or RGBA in sRGB gamma-space,
        as either 0-1 floats or 0-255 integers, with separate alpha.
    radii:
        Optional radii (make it a sphere).
    labels:
        Optional per-point text to show with the points
    class_ids:
        Optional class ids for the points.
        The class id provides colors and labels if not specified explicitly.
        See [rerun.log_annotation_context][]
    keypoint_ids:
        Optional key point ids for the points, identifying them within a class.
        If keypoint_ids are passed in but no class_ids were specified, class_id will be set to 0.
        This is useful to identify points within a single classification (which is identified with class_id).
        E.g. the classification might be 'Person' and the keypoints refer to joints on a detected skeleton.
        See [rerun.log_annotation_context][]
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the points will be timeless (default: False).

    """

    if keypoint_ids is not None and class_ids is None:
        class_ids = 0
    if positions is None:
        positions = np.require([], dtype="float32")
    else:
        positions = np.require(positions, dtype="float32")

    colors = _normalize_colors(colors)
    radii = _normalize_radii(radii)
    labels = _normalize_labels(labels)
    class_ids = _normalize_ids(class_ids)
    keypoint_ids = _normalize_ids(keypoint_ids)

    identifiers_np = np.array((), dtype="uint64")
    if identifiers is not None:
        try:
            identifiers_np = np.require(identifiers, dtype="uint64")
        except ValueError:
            _send_warning("Only integer identifiers supported", 1)

    # 0 = instanced, 1 = splat
    comps = [{}, {}]  # type: ignore[var-annotated]

    if positions.any():
        if positions.shape[1] == 2:
            comps[0]["rerun.point2d"] = Point2DArray.from_numpy(positions)
        elif positions.shape[1] == 3:
            comps[0]["rerun.point3d"] = Point3DArray.from_numpy(positions)
        else:
            raise TypeError("Positions should be either Nx2 or Nx3")

    if len(identifiers_np):
        comps[0]["rerun.instance_key"] = InstanceArray.from_numpy(identifiers_np)

    if len(colors):
        is_splat = len(colors.shape) == 1
        if is_splat:
            colors = colors.reshape(1, len(colors))
        comps[is_splat]["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if len(radii):
        is_splat = len(radii) == 1
        comps[is_splat]["rerun.radius"] = RadiusArray.from_numpy(radii)

    if len(labels):
        is_splat = len(labels) == 1
        comps[is_splat]["rerun.label"] = LabelArray.new(labels)

    if len(class_ids):
        is_splat = len(class_ids) == 1
        comps[is_splat]["rerun.class_id"] = ClassIdArray.from_numpy(class_ids)

    if len(keypoint_ids):
        is_splat = len(keypoint_ids) == 1
        comps[is_splat]["rerun.keypoint_id"] = ClassIdArray.from_numpy(keypoint_ids)

    if ext:
        _add_extension_components(comps[0], comps[1], ext, identifiers_np)

    if comps[1]:
        comps[1]["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=comps[1], timeless=timeless)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    bindings.log_arrow_msg(entity_path, components=comps[0], timeless=timeless)
