from __future__ import annotations

import colorsys
import math
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import Color

_GOLDEN_RATIO = (math.sqrt(5.0) - 1.0) / 2.0


class ColorExt:
    """Extension for [Color][rerun.components.Color]."""

    @staticmethod
    def from_string(s: str) -> Color:
        """
        Generate a random yet deterministic color based on a string.

        The color is guaranteed to be identical for the same input string.
        """

        from . import Color

        # adapted from egui::PlotUi
        hue = (hash(s) & 0xFFFF) / 2**16 * _GOLDEN_RATIO
        return Color([round(comp * 255) for comp in colorsys.hsv_to_rgb(hue, 0.85, 0.5)])
