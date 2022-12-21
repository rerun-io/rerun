import logging
from typing import Optional, Sequence

import numpy.typing as npt
from rerun.log import EXP_ARROW, _to_sequence

from rerun import bindings

__all__ = [
    "log_obb",
]


def log_obb(
    obj_path: str,
    half_size: Optional[npt.ArrayLike],
    position: Optional[npt.ArrayLike] = None,
    rotation_q: Optional[npt.ArrayLike] = None,
    color: Optional[Sequence[int]] = None,
    stroke_width: Optional[float] = None,
    label: Optional[str] = None,
    class_id: Optional[int] = None,
    timeless: bool = False,
) -> None:
    """
    Log a 3D oriented bounding box, defined by its half size.

    `half_size`: Array with [x, y, z] half dimensions of the OBB.
    `position`: Array with [x, y, z] position of the OBB in world space.
    `rotation_q`: Array with quaternion coordinates [x, y, z, w] for the rotation from model to world space
    `color`: Optional RGB or RGBA triplet in 0-255 sRGB.
    `stroke_width`: Optional width of the OBB edges.
    `label` Optional text label placed at `position`.
    `class_id`: Optional class id for the OBB.
                 The class id provides colors and labels if not specified explicitly.
    """
    if EXP_ARROW.classic_log_gate():
        bindings.log_obb(
            obj_path,
            half_size=_to_sequence(half_size),
            position=_to_sequence(position),
            rotation_q=_to_sequence(rotation_q),
            color=color,
            stroke_width=stroke_width,
            label=label,
            timeless=timeless,
            class_id=class_id,
        )

    if EXP_ARROW.arrow_log_gate():
        logging.warning("log_obb() not yet implemented for Arrow.")
