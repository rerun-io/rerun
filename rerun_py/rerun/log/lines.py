from typing import Optional, Sequence

import numpy as np
import numpy.typing as npt
from rerun.components.color import ColorRGBAArray
from rerun.components.instance import InstanceArray
from rerun.components.linestrip import LineStrip2DArray, LineStrip3DArray
from rerun.components.radius import RadiusArray
from rerun.log import _normalize_colors, _normalize_radii

from rerun import bindings

__all__ = [
    "log_path",
    "log_line_segments",
]


def log_path(
    entity_path: str,
    positions: Optional[npt.NDArray[np.float32]],
    *,
    stroke_width: Optional[float] = None,
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
) -> None:
    r"""
    Log a 3D path.

    A path is a list of points connected by line segments. It can be used to draw approximations of smooth curves.

    The points will be connected in order, like so:

           2------3     5
          /        \   /
    0----1          \ /
                     4

    `positions`: a Nx3 array of points along the path.
    `stroke_width`: width of the line.
    `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    """
    if positions is not None:
        positions = np.require(positions, dtype="float32")

    comps = {}

    if positions is not None:
        comps["rerun.linestrip3d"] = LineStrip3DArray.from_numpy_arrays([positions])

    if color:
        colors = _normalize_colors([color])
        comps["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if stroke_width:
        radii = _normalize_radii([stroke_width / 2])
        comps["rerun.radius"] = RadiusArray.from_numpy(radii)

    bindings.log_arrow_msg(entity_path, components=comps, timeless=timeless)


def log_line_segments(
    entity_path: str,
    positions: npt.NDArray[np.float32],
    *,
    stroke_width: Optional[float] = None,
    color: Optional[Sequence[int]] = None,
    timeless: bool = False,
) -> None:
    r"""
    Log many 2D or 3D line segments.

    The points will be connected in even-odd pairs, like so:

           2------3     5
                       /
    0----1            /
                     4

    `positions`: a Nx3 array of points along the path.
    `stroke_width`: width of the line.
    `color` is optional RGB or RGBA triplet in 0-255 sRGB.
    """
    if positions is None:
        positions = np.require([], dtype="float32")
    positions = np.require(positions, dtype="float32")

    # 0 = instanced, 1 = splat
    comps = [{}, {}]  # type: ignore[var-annotated]

    if positions is not None:
        # If not a multiple of 2, drop the last row
        if len(positions) % 2:
            positions = positions[:-1]
        if positions.shape[1] == 2:
            # Reshape even-odd pairs into a collection of line-strips of length2
            # [[a00, a01], [a10, a11], [b00, b01], [b10, b11]]
            # -> [[[a00, a01], [a10, a11]], [[b00, b01], [b10, b11]]]
            positions = positions.reshape([len(positions) // 2, 2, 2])
            comps[0]["rerun.linestrip2d"] = LineStrip2DArray.from_numpy_arrays(positions)
        elif positions.shape[1] == 3:
            # Same as above but for 3d points
            positions = positions.reshape([len(positions) // 2, 2, 3])
            comps[0]["rerun.linestrip3d"] = LineStrip3DArray.from_numpy_arrays(positions)
        else:
            raise TypeError("Positions should be either Nx2 or Nx3")

    # The current API splats both color and stroke-width, though the data-model doesn't
    # require that we do so.
    if color:
        colors = _normalize_colors([color])
        comps[1]["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if stroke_width:
        radii = _normalize_radii([stroke_width / 2])
        comps[1]["rerun.radius"] = RadiusArray.from_numpy(radii)

    bindings.log_arrow_msg(entity_path, components=comps[0], timeless=timeless)

    if comps[1]:
        comps[1]["rerun.instance"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=comps[1], timeless=timeless)
