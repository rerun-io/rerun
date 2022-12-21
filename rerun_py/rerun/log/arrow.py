from typing import Optional, Sequence

import numpy.typing as npt
from rerun.log import _to_sequence

from rerun import bindings

__all__ = [
    "log_arrow",
]


def log_arrow(
    obj_path: str,
    origin: Optional[npt.ArrayLike],
    vector: Optional[npt.ArrayLike] = None,
    *,
    color: Optional[Sequence[int]] = None,
    label: Optional[str] = None,
    width_scale: Optional[float] = None,
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
    obj_path
        The path to store the object at.
    origin
        The base position of the arrow.
    vector
        The vector along which the arrow will be drawn.
    color
        An optional RGB or RGBA triplet in 0-255 sRGB.
    label
        An optional text to show beside the arrow.
    width_scale
        An optional scaling factor, default=1.0.
    timeless
        Object is not time-dependent, and will be visible at any time point.

    """
    bindings.log_arrow(
        obj_path,
        origin=_to_sequence(origin),
        vector=_to_sequence(vector),
        color=color,
        label=label,
        width_scale=width_scale,
        timeless=timeless,
    )
