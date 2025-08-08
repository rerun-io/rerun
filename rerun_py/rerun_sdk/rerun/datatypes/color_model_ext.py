from __future__ import annotations


class ColorModelExt:
    """Extension for [ColorModel][rerun.datatypes.ColorModel]."""

    def num_channels(self) -> int:
        from . import ColorModel

        """Returns the number of channels for this color model."""
        if self == ColorModel.L:
            return 1
        elif self in (ColorModel.RGB, ColorModel.BGR):
            return 3
        elif self in (ColorModel.RGBA, ColorModel.BGRA):
            return 4
        else:
            raise ValueError(f"Unknown color model: {self}")
