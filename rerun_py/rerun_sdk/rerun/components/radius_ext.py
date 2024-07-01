from __future__ import annotations

import numbers
from typing import TYPE_CHECKING

import numpy as np
import numpy.typing as npt

if TYPE_CHECKING:
    from . import RadiusArrayLike


class RadiusExt:
    """Extension for [Radius][rerun.components.Radius]."""

    @staticmethod
    def ui_points(radii: numbers.Number | npt.ArrayLike) -> RadiusArrayLike:
        """
        Create a radius or list of radii in UI points.

        By default, radii are interpreted as scene units.
        Ui points on the other hand are independent of zooming in Views, but are sensitive to the application ui scaling.
        At 100% ui scaling, ui points are equal to pixels
        The Viewer's ui scaling defaults to the OS scaling which typically is 100% for full HD screens and 200% for 4k screens.

        Internally, ui radii are stored as negative values.
        Therefore, all this method does is to ensure that all returned values are negative.
        """
        return -np.abs(np.array(radii, dtype=np.float32))
