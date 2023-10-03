from __future__ import annotations

from fractions import Fraction
from typing import TYPE_CHECKING

import numpy as np

if TYPE_CHECKING:
    from . import Scale3DLike, Vec3D


class Scale3DExt:
    """Extension for [Scale3D][rerun.datatypes.Scale3D]."""

    @staticmethod
    def inner__field_converter_override(data: Scale3DLike) -> Vec3D | float:
        from . import Scale3D, Vec3D

        if isinstance(data, Vec3D):
            return data
        elif isinstance(data, Scale3D):
            return data.inner
        elif isinstance(data, (float, int, Fraction)):
            return float(data)
        else:
            return Vec3D(np.array(data))
