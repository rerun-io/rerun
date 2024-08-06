# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/colormap.fbs".

# You can extend this class by creating a "ColormapExt" class in "colormap_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = ["Colormap", "ColormapArrayLike", "ColormapBatch", "ColormapLike", "ColormapType"]


from enum import Enum


class Colormap(Enum):
    """
    **Component**: Colormap for mapping scalar values within a given range to a color.

    This provides a number of popular pre-defined colormaps.
    In the future, the Rerun Viewer will allow users to define their own colormaps,
    but currently the Viewer is limited to the types defined here.
    """

    Grayscale = 0
    """
    A simple black to white gradient.

    This is a sRGB gray gradient which is perceptually uniform.
    """

    Inferno = 1
    """
    The Inferno colormap from Matplotlib.

    This is a perceptually uniform colormap.
    It interpolates from black to red to bright yellow.
    """

    Magma = 2
    """
    The Magma colormap from Matplotlib.

    This is a perceptually uniform colormap.
    It interpolates from black to purple to white.
    """

    Plasma = 3
    """
    The Plasma colormap from Matplotlib.

    This is a perceptually uniform colormap.
    It interpolates from dark blue to purple to yellow.
    """

    Turbo = 4
    """
    Google's Turbo colormap map.

    This is a perceptually non-uniform rainbow colormap addressing many issues of
    more traditional rainbow colormaps like Jet.
    It is more perceptually uniform without sharp transitions and is more colorblind-friendly.
    Details: <https://research.google/blog/turbo-an-improved-rainbow-colormap-for-visualization/>
    """

    Viridis = 5
    """
    The Viridis colormap from Matplotlib

    This is a perceptually uniform colormap which is robust to color blindness.
    It interpolates from dark purple to green to yellow.
    """

    CyanToYellow = 6
    """
    Rasmusgo's Cyan to Yellow colormap

    This is a perceptually uniform colormap which is robust to color blindness.
    It is especially suited for visualizing signed values.
    It interpolates from cyan to blue to dark gray to brass to yellow.
    """

    @classmethod
    def auto(cls, val: str | int | Colormap) -> Colormap:
        """Best-effort converter."""
        if isinstance(val, Colormap):
            return val
        if isinstance(val, int):
            return cls(val)
        try:
            return cls[val]
        except KeyError:
            val_lower = val.lower()
            for variant in cls:
                if variant.name.lower() == val_lower:
                    return variant
        raise ValueError(f"Cannot convert {val} to {cls.__name__}")

    def __str__(self) -> str:
        """Returns the variant name."""
        return self.name


ColormapLike = Union[
    Colormap,
    Literal[
        "CyanToYellow",
        "Grayscale",
        "Inferno",
        "Magma",
        "Plasma",
        "Turbo",
        "Viridis",
        "cyantoyellow",
        "grayscale",
        "inferno",
        "magma",
        "plasma",
        "turbo",
        "viridis",
    ],
    int,
]
ColormapArrayLike = Union[ColormapLike, Sequence[ColormapLike]]


class ColormapType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.Colormap"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.uint8(), self._TYPE_NAME)


class ColormapBatch(BaseBatch[ColormapArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = ColormapType()

    @staticmethod
    def _native_to_pa_array(data: ColormapArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (Colormap, int, str)):
            data = [data]

        pa_data = [Colormap.auto(v).value if v else None for v in data]

        return pa.array(pa_data, type=data_type)
