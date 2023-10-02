# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/color.fbs".

# You can extend this class by creating a "ColorExt" class in "color_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["Color", "ColorBatch", "ColorType"]


class Color(datatypes.Rgba32):
    """
    An RGBA color with unmultiplied/separate alpha, in sRGB gamma space with linear alpha.

    The color is stored as a 32-bit integer, where the most significant
    byte is `R` and the least significant byte is `A`.

    Float colors are assumed to be in 0-1 gamma sRGB space.
    All other colors are assumed to be in 0-255 gamma sRGB space.
    If there is an alpha, we assume it is in linear space, and separate (NOT pre-multiplied).
    """

    # You can define your own __init__ function as a member of ColorExt in color_ext.py

    # Note: there are no fields here because Color delegates to datatypes.Rgba32
    pass


class ColorType(datatypes.Rgba32Type):
    _TYPE_NAME: str = "rerun.components.Color"


class ColorBatch(datatypes.Rgba32Batch, ComponentBatchMixin):
    _ARROW_TYPE = ColorType()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ColorType())
