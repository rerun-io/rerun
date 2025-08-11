from __future__ import annotations

import math
from typing import Any


class AngleExt:
    """Extension for [Angle][rerun.datatypes.Angle]."""

    def __init__(self: Any, rad: float | None = None, deg: float | None = None) -> None:
        """
        Create a new instance of the Angle datatype.

        Parameters
        ----------
        rad:
            Angle in radians, specify either `rad` or `deg`.
        deg:
            Angle in degrees, specify either `rad` or `deg`.
            Converts the angle to radians internally.

        """

        if rad is not None:
            self.radians = rad
        elif deg is not None:
            self.radians = math.radians(deg)
        else:
            raise ValueError("Either `rad` or `deg` must be provided.")
