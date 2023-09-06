# DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/python.rs:277.

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from ._overrides import depthimage_data_converter  # noqa: F401

__all__ = ["DepthImage"]


@define(str=False, repr=False)
class DepthImage(Archetype):
    """
    A depth image.

    The shape of the `TensorData` must be mappable to an `HxW` tensor.
    Each pixel corresponds to a depth value in units specified by meter.

    Example
    -------
    ```python

    import numpy as np
    import rerun as rr
    import rerun.experimental as rr2

    # Create an image with numpy
    image = np.zeros((200, 300, 3), dtype=np.uint8)
    image[:, :, 0] = 255
    image[50:150, 50:150] = (0, 255, 0)

    rr.init("rerun_example_images", spawn=True)

    rr2.log("simple", rr2.Image(image))
    ```
    """

    data: components.TensorDataArray = field(metadata={"component": "primary"}, converter=depthimage_data_converter)
    """
    The depth-image data. Should always be a rank-2 tensor.
    """

    meter: components.DepthMeterArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.DepthMeterArray.from_similar,  # type: ignore[misc]
    )
    """
    An optional floating point value that specifies how long a meter is in the native depth units.

    For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
    and a range of up to ~65 meters (2^16 / 1000).
    """

    draw_order: components.DrawOrderArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.DrawOrderArray.from_similar,  # type: ignore[misc]
    )
    """
    An optional floating point value that specifies the 2D drawing order.
    Objects with higher values are drawn on top of those with lower values.

    The default for 2D points is -10.0.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
