# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/image.fbs".

# You can extend this class by creating a "ImageExt" class in "image_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from ..error_utils import catch_and_log_exceptions
from .image_ext import ImageExt

__all__ = ["Image"]


@define(str=False, repr=False, init=False)
class Image(ImageExt, Archetype):
    """
    **Archetype**: A monochrome or color image.

    The order of dimensions in the underlying [`components.TensorData`][rerun.components.TensorData] follows the typical
    row-major, interleaved-pixel image format. Additionally, Rerun orders the
    [`datatypes.TensorDimension`][rerun.datatypes.TensorDimension]s within the shape description from outer-most to inner-most.

    As such, the shape of the [`components.TensorData`][rerun.components.TensorData] must be mappable to:
    - A `HxW` tensor, treated as a grayscale image.
    - A `HxWx3` tensor, treated as an RGB image.
    - A `HxWx4` tensor, treated as an RGBA image.

    Leading and trailing unit-dimensions are ignored, so that
    `1x480x640x3x1` is treated as a `480x640x3` RGB image.

    Rerun also supports compressed image encoded as JPEG, N12, and YUY2.
    Using these formats can save a lot of bandwidth and memory.
    To compress an image, use [`rerun.Image.compress`][].
    To pass in an already encoded image, use  [`rerun.ImageEncoded`][].

    Example
    -------
    ### `image_simple`:
    ```python
    import numpy as np
    import rerun as rr

    # Create an image with numpy
    image = np.zeros((200, 300, 3), dtype=np.uint8)
    image[:, :, 0] = 255
    image[50:150, 50:150] = (0, 255, 0)

    rr.init("rerun_example_image", spawn=True)

    rr.log("image", rr.Image(image))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1200w.png">
      <img src="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(
        self: Any,
        data: datatypes.TensorDataLike,
        *,
        opacity: datatypes.Float32Like | None = None,
        draw_order: datatypes.Float32Like | None = None,
    ):
        """
        Create a new instance of the Image archetype.

        Parameters
        ----------
        data:
            The image data. Should always be a 2- or 3-dimensional tensor.
        opacity:
            Opacity of the image, useful for layering several images.

            Defaults to 1.0 (fully opaque).
        draw_order:
            An optional floating point value that specifies the 2D drawing order.

            Objects with higher values are drawn on top of those with lower values.

        """

        # You can define your own __init__ function as a member of ImageExt in image_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(data=data, opacity=opacity, draw_order=draw_order)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            data=None,  # type: ignore[arg-type]
            opacity=None,  # type: ignore[arg-type]
            draw_order=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Image:
        """Produce an empty Image, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    data: components.TensorDataBatch = field(
        metadata={"component": "required"},
        converter=ImageExt.data__field_converter_override,  # type: ignore[misc]
    )
    # The image data. Should always be a 2- or 3-dimensional tensor.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    opacity: components.OpacityBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.OpacityBatch._optional,  # type: ignore[misc]
    )
    # Opacity of the image, useful for layering several images.
    #
    # Defaults to 1.0 (fully opaque).
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    draw_order: components.DrawOrderBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.DrawOrderBatch._optional,  # type: ignore[misc]
    )
    # An optional floating point value that specifies the 2D drawing order.
    #
    # Objects with higher values are drawn on top of those with lower values.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
