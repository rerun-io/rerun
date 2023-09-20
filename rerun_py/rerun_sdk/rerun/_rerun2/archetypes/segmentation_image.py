# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/segmentation_image.fbs".

# You can extend this class by creating a "SegmentationImageExt" class in "segmentation_image_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from .segmentation_image_ext import SegmentationImageExt

__all__ = ["SegmentationImage"]


@define(str=False, repr=False)
class SegmentationImage(SegmentationImageExt, Archetype):
    """
    An image made up of integer class-ids.

    The shape of the `TensorData` must be mappable to an `HxW` tensor.
    Each pixel corresponds to a depth value in units specified by meter.

    Leading and trailing unit-dimensions are ignored, so that
    `1x640x480x1` is treated as a `640x480` image.

    Examples
    --------
    ```python

    import numpy as np
    import rerun as rr
    import rerun.experimental as rr2

    # Create a segmentation image
    image = np.zeros((8, 12), dtype=np.uint8)
    image[0:4, 0:6] = 1
    image[4:8, 6:12] = 2

    rr.init("rerun_example_segmentation_image", spawn=True)

    # Assign a label and color to each class
    rr2.log("/", rr2.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]))

    # TODO(#2792): SegmentationImage archetype
    rr.log_segmentation_image("image", np.array(image))
    ```
    """

    # You can define your own __init__ function as a member of SegmentationImageExt in segmentation_image_ext.py

    data: components.TensorDataArray = field(
        metadata={"component": "required"},
        converter=SegmentationImageExt.data__field_converter_override,  # type: ignore[misc]
    )
    """
    The image data. Should always be a rank-2 tensor.
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
