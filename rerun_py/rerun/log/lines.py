from typing import Optional, Sequence

import numpy as np
import numpy.typing as npt

from rerun import rerun_bindings  # type: ignore[attr-defined]

__all__ = [
    "log_path",
    "log_line_segments",
]


def log_path(
    obj_path: str,
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
    rerun_bindings.log_path(obj_path, positions, stroke_width, color, timeless)


def log_line_segments(
    obj_path: str,
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
        positions = []
    positions = np.require(positions, dtype="float32")
    rerun_bindings.log_line_segments(obj_path, positions, stroke_width, color, timeless)
