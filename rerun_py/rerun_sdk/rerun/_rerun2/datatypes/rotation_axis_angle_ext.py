from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import Angle, AngleLike


class RotationAxisAngleExt:
    # needed because the default converter doesn't handle well Angle, which has an overridden __init__
    @staticmethod
    def angle__field_converter_override(x: AngleLike) -> Angle:
        from . import Angle

        if isinstance(x, Angle):
            return x
        else:
            return Angle(rad=x)
