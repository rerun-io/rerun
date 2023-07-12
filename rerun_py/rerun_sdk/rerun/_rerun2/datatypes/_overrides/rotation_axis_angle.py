from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .. import Angle, AngleLike


# needed because the default converter doesn't handle well Angle, which has an overridden __init__
def rotationaxisangle_angle_converter(x: AngleLike) -> Angle:
    from .. import Angle

    if isinstance(x, Angle):
        return x
    else:
        return Angle(rad=x)
