from typing import Any, Dict, Optional

import numpy as np
import numpy.typing as npt
from deprecated import deprecated

from depthai_viewer import bindings
from depthai_viewer.components.color import ColorRGBAArray
from depthai_viewer.components.instance import InstanceArray
from depthai_viewer.components.linestrip import LineStrip2DArray, LineStrip3DArray
from depthai_viewer.components.radius import RadiusArray
from depthai_viewer.log import Color, _normalize_colors, _normalize_radii
from depthai_viewer.log.extension_components import _add_extension_components
from depthai_viewer.log.log_decorator import log_decorator

__all__ = [
    "log_path",
    "log_line_strip",
    "log_line_segments",
]


@deprecated(version="0.2.0", reason="Use log_line_strip instead")
def log_path(
    entity_path: str,
    positions: Optional[npt.ArrayLike],
    *,
    stroke_width: Optional[float] = None,
    color: Optional[Color] = None,
    ext: Optional[Dict[str, Any]] = None,
    timeless: bool = False,
) -> None:
    log_line_strip(entity_path, positions, stroke_width=stroke_width, color=color, ext=ext, timeless=timeless)


@log_decorator
def log_line_strip(
    entity_path: str,
    positions: Optional[npt.ArrayLike],
    *,
    stroke_width: Optional[float] = None,
    color: Optional[Color] = None,
    ext: Optional[Dict[str, Any]] = None,
    timeless: bool = False,
) -> None:
    r"""
    Log a line strip through 2D or 3D space.

    A line strip is a list of points connected by line segments. It can be used to draw approximations of smooth curves.

    The points will be connected in order, like so:
    ```
           2------3     5
          /        \   /
    0----1          \ /
                     4
    ```

    Parameters
    ----------
    entity_path:
        Path to the path in the space hierarchy
    positions:
        An Nx2 or Nx3 array of points along the path.
    stroke_width:
        Optional width of the line.
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the path will be timeless (default: False).

    """

    if positions is not None:
        positions = np.require(positions, dtype="float32")

    instanced: Dict[str, Any] = {}
    splats: Dict[str, Any] = {}

    if positions is not None:
        if positions.shape[1] == 2:
            instanced["rerun.linestrip2d"] = LineStrip2DArray.from_numpy_arrays([positions])
        elif positions.shape[1] == 3:
            instanced["rerun.linestrip3d"] = LineStrip3DArray.from_numpy_arrays([positions])
        else:
            raise TypeError("Positions should be either Nx2 or Nx3")

    if color:
        colors = _normalize_colors([color])
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if stroke_width:
        radii = _normalize_radii([stroke_width / 2])
        instanced["rerun.radius"] = RadiusArray.from_numpy(radii)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless)


@log_decorator
def log_line_segments(
    entity_path: str,
    positions: npt.ArrayLike,
    *,
    stroke_width: Optional[float] = None,
    color: Optional[Color] = None,
    ext: Optional[Dict[str, Any]] = None,
    timeless: bool = False,
) -> None:
    r"""
    Log many 2D or 3D line segments.

    The points will be connected in even-odd pairs, like so:

    ```
           2------3     5
                       /
    0----1            /
                     4
    ```

    Parameters
    ----------
    entity_path:
        Path to the line segments in the space hierarchy
    positions:
        An Nx2 or Nx3 array of points. Even-odd pairs will be connected as segments.
    stroke_width:
        Optional width of the line.
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the line segments will be timeless (default: False).

    """

    if positions is None:
        positions = np.require([], dtype="float32")
    positions = np.require(positions, dtype="float32")

    instanced: Dict[str, Any] = {}
    splats: Dict[str, Any] = {}

    if positions is not None:
        # If not a multiple of 2, drop the last row
        if len(positions) % 2:
            positions = positions[:-1]
        if positions.shape[1] == 2:
            # Reshape even-odd pairs into a collection of line-strips of length2
            # [[a00, a01], [a10, a11], [b00, b01], [b10, b11]]
            # -> [[[a00, a01], [a10, a11]], [[b00, b01], [b10, b11]]]
            positions = positions.reshape([len(positions) // 2, 2, 2])
            instanced["rerun.linestrip2d"] = LineStrip2DArray.from_numpy_arrays(positions)
        elif positions.shape[1] == 3:
            # Same as above but for 3d points
            positions = positions.reshape([len(positions) // 2, 2, 3])
            instanced["rerun.linestrip3d"] = LineStrip3DArray.from_numpy_arrays(positions)
        else:
            raise TypeError("Positions should be either Nx2 or Nx3")

    # The current API splats both color and stroke-width, though the data-model doesn't
    # require that we do so.
    if color:
        colors = _normalize_colors([color])
        splats["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if stroke_width:
        radii = _normalize_radii([stroke_width / 2])
        splats["rerun.radius"] = RadiusArray.from_numpy(radii)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless)
