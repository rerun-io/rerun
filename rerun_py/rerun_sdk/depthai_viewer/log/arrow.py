from typing import Any, Dict, Optional

import numpy as np
import numpy.typing as npt

from depthai_viewer import bindings
from depthai_viewer.components.arrow import Arrow3DArray
from depthai_viewer.components.color import ColorRGBAArray
from depthai_viewer.components.instance import InstanceArray
from depthai_viewer.components.label import LabelArray
from depthai_viewer.components.radius import RadiusArray
from depthai_viewer.log import Color, _normalize_colors, _normalize_radii
from depthai_viewer.log.extension_components import _add_extension_components
from depthai_viewer.log.log_decorator import log_decorator

__all__ = [
    "log_arrow",
]


@log_decorator
def log_arrow(
    entity_path: str,
    origin: Optional[npt.ArrayLike],
    vector: Optional[npt.ArrayLike] = None,
    *,
    color: Optional[Color] = None,
    label: Optional[str] = None,
    width_scale: Optional[float] = None,
    ext: Optional[Dict[str, Any]] = None,
    timeless: bool = False,
) -> None:
    """
    Log a 3D arrow.

    An arrow is defined with an `origin`, and a `vector`. This can also be considered as `start` and `end` positions
    for the arrow.

    The shaft is rendered as a cylinder with `radius = 0.5 * width_scale`.
    The tip is rendered as a cone with `height = 2.0 * width_scale` and `radius = 1.0 * width_scale`.

    Parameters
    ----------
    entity_path
        The path to store the entity at.
    origin
        The base position of the arrow.
    vector
        The vector along which the arrow will be drawn.
    color
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    label
        An optional text to show beside the arrow.
    width_scale
        An optional scaling factor, default=1.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless
        The entity is not time-dependent, and will be visible at any time point.

    """

    instanced: Dict[str, Any] = {}
    splats: Dict[str, Any] = {}

    if origin is not None:
        if vector is None:
            raise TypeError("Must provide both origin and vector")
        origin = np.require(origin, dtype="float32")
        vector = np.require(vector, dtype="float32")
        instanced["rerun.arrow3d"] = Arrow3DArray.from_numpy(origin.reshape(1, 3), vector.reshape(1, 3))

    if color:
        colors = _normalize_colors([color])
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if label:
        instanced["rerun.label"] = LabelArray.new([label])

    if width_scale:
        radii = _normalize_radii([width_scale / 2])
        instanced["rerun.radius"] = RadiusArray.from_numpy(radii)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless)
