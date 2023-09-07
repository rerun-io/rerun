# DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/python.rs:277.

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from ._overrides import image_data_converter  # noqa: F401

__all__ = ["Image"]


@define(str=False, repr=False)
class Image(Archetype):
    """
    A monochrome or color image.

    The shape of the `TensorData` must be mappable to:
    - A `HxW` tensor, treated as a grayscale image.
    - A `HxWx3` tensor, treated as an RGB image.
    - A `HxWx4` tensor, treated as an RGBA image.

    Leading and trailing unit-dimensions are ignored, so that
    `1x640x480x3x1` is treated as a `640x480x3` RGB image.

    Example
    -------
    ```python

    import numpy as np
    import rerun as rr
    import rerun.experimental as rr2

    # Create an image with numpy
    image = np.zeros((8, 12, 3), dtype=np.uint8)
    image[:, :, 0] = 255
    image[0:4, 0:6] = (0, 255, 0)

    rr.init("rerun_example_image_simple", spawn=True)

    rr2.log("simple", rr2.Image(image))
    ```
    """

    data: components.TensorDataArray = field(metadata={"component": "primary"}, converter=image_data_converter)
    """
    The image data. Should always be a rank-2 or rank-3 tensor.
    """

    draw_order: components.DrawOrderArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.DrawOrderArray.from_similar,  # type: ignore[misc]
    )
    """
    An optional floating point value that specifies the 2D drawing order.
    Objects with higher values are drawn on top of those with lower values.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
