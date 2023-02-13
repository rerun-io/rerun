from typing import Any, Dict, Optional, Sequence

import numpy as np
import numpy.typing as npt
from rerun.components.annotation import ClassIdArray
from rerun.components.box import Box3DArray
from rerun.components.color import ColorRGBAArray
from rerun.components.instance import InstanceArray
from rerun.components.label import LabelArray
from rerun.components.quaternion import QuaternionArray
from rerun.components.radius import RadiusArray
from rerun.components.vec import Vec3DArray
from rerun.log import _normalize_colors, _normalize_ids, _normalize_radii
from rerun.log.extension_components import _add_extension_components

from rerun import bindings

__all__ = [
    "log_obb",
]


def log_obb(
    entity_path: str,
    half_size: Optional[npt.ArrayLike],
    position: Optional[npt.ArrayLike] = None,
    rotation_q: Optional[npt.ArrayLike] = None,
    color: Optional[Sequence[int]] = None,
    stroke_width: Optional[float] = None,
    label: Optional[str] = None,
    class_id: Optional[int] = None,
    ext: Optional[Dict[str, Any]] = None,
    timeless: bool = False,
) -> None:
    """
    Log a 3D oriented bounding box.

    Parameters
    ----------
    entity_path:
        The path to the oriented bounding box in the space hierarchy.
    half_size:
        Array with [x, y, z] half dimensions of the OBB.
    position:
        Array with [x, y, z] position of the OBB in world space.
    rotation_q:
        Array with quaternion coordinates [x, y, z, w] for the rotation from model to world space.
    color:
        Optional RGB or RGBA triplet in 0-255 sRGB.
    stroke_width:
        Optional width of the OBB edges.
    label:
        Optional text label placed at `position`.
    class_id:
        Optional class id for the OBB.  The class id provides colors and labels if not specified explicitly.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the bounding box will be timeless (default: False).

    """

    if not bindings.is_enabled():
        return

    instanced: Dict[str, Any] = {}
    splats: Dict[str, Any] = {}

    if half_size is not None:
        size = np.require(half_size, dtype="float32")

        if size.shape[0] == 3:
            instanced["rerun.box3d"] = Box3DArray.from_numpy(size.reshape(1, 3))
        else:
            raise TypeError("Position should be 1x3")

    if position is not None:
        position = np.require(position, dtype="float32")

        if position.shape[0] == 3:
            instanced["rerun.vec3d"] = Vec3DArray.from_numpy(position.reshape(1, 3))
        else:
            raise TypeError("Position should be 1x3")

    if rotation_q is not None:
        rotation = np.require(rotation_q, dtype="float32")

        if rotation.shape[0] == 4:
            instanced["rerun.quaternion"] = QuaternionArray.from_numpy(rotation.reshape(1, 4))
        else:
            raise TypeError("Rotation should be 1x4")

    if color:
        colors = _normalize_colors([color])
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if stroke_width:
        radii = _normalize_radii([stroke_width / 2])
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
