# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/depth_image.fbs".

# You can extend this class by creating a "DepthImageExt" class in "depth_image_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from .depth_image_ext import DepthImageExt

__all__ = ["DepthImage"]


@define(str=False, repr=False)
class DepthImage(DepthImageExt, Archetype):
    """
    A depth image.

    The shape of the `TensorData` must be mappable to an `HxW` tensor.
    Each pixel corresponds to a depth value in units specified by `meter`.

    Example
    -------
    ```python

    import numpy as np
    import rerun as rr
    import rerun.experimental as rr2

    # Create a dummy depth image
    image = 65535 * np.ones((8, 12), dtype=np.uint16)
    image[0:4, 0:6] = 20000
    image[4:8, 6:12] = 45000

    rr.init("rerun_example_depth_image", spawn=True)

    # Log the tensor, assigning names to each dimension
    rr2.log("depth", rr2.DepthImage(image, meter=10_000.0))
    ```
    """

    # You can define your own __init__ function as a member of DepthImageExt in depth_image_ext.py

    data: components.TensorDataArray = field(
        metadata={"component": "required"},
        converter=DepthImageExt.data__field_converter_override,  # type: ignore[misc]
    )
    """
    The depth-image data. Should always be a rank-2 tensor.
    """

    meter: components.DepthMeterArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.DepthMeterArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    An optional floating point value that specifies how long a meter is in the native depth units.

    For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
    and a range of up to ~65 meters (2^16 / 1000).
    """

    draw_order: components.DrawOrderArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.DrawOrderArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    An optional floating point value that specifies the 2D drawing order.
    Objects with higher values are drawn on top of those with lower values.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
