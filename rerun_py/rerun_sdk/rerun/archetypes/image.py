# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/image.fbs".

# You can extend this class by creating a "ImageExt" class in "image_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from .image_ext import ImageExt

__all__ = ["Image"]


@define(str=False, repr=False, init=False)
class Image(ImageExt, Archetype):
    """
    **Archetype**: A monochrome or color image.

    See also [`archetypes.DepthImage`][rerun.archetypes.DepthImage] and [`archetypes.SegmentationImage`][rerun.archetypes.SegmentationImage].

    The raw image data is stored as a single buffer of bytes in a [`components.Blob`][rerun.components.Blob].
    The meaning of these bytes is determined by the [`components.ImageFormat`][rerun.components.ImageFormat] which specifies the resolution
    and the pixel format (e.g. RGB, RGBA, …).

    The order of dimensions in the underlying [`components.Blob`][rerun.components.Blob] follows the typical
    row-major, interleaved-pixel image format.

    Rerun also supports compressed images (JPEG, PNG, …), using [`archetypes.EncodedImage`][rerun.archetypes.EncodedImage].
    Compressing images can save a lot of bandwidth and memory.

    Examples
    --------
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

    ### Advanced usage of `send_columns` to send multiple images at once:
    ```python
    import numpy as np
    import rerun as rr

    rr.init("rerun_example_image_send_columns", spawn=True)

    # Timeline on which the images are distributed.
    times = np.arange(0, 20)

    # Create a batch of images with a moving rectangle.
    width, height = 300, 200
    images = np.zeros((len(times), height, width, 3), dtype=np.uint8)
    images[:, :, :, 2] = 255
    for t in times:
        images[t, 50:150, (t * 10) : (t * 10 + 100), 1] = 255

    # Log the ImageFormat and indicator once, as static.
    format_static = rr.components.ImageFormat(width=width, height=height, color_model="RGB", channel_datatype="U8")
    rr.log("images", [format_static, rr.Image.indicator()], static=True)

    # Send all images at once.
    rr.send_columns(
        "images",
        times=[rr.TimeSequenceColumn("step", times)],
        # Reshape the images so `ImageBufferBatch` can tell that this is several blobs.
        #
        # Note that the `ImageBufferBatch` consumes arrays of bytes,
        # so if you have a different channel datatype than `U8`, you need to make sure
        # that the data is converted to arrays of bytes before passing it to `ImageBufferBatch`.
        components=[rr.components.ImageBufferBatch(images.reshape(len(times), -1))],
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/image_send_columns/321455161d79e2c45d6f5a6f175d6f765f418897/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/image_send_columns/321455161d79e2c45d6f5a6f175d6f765f418897/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/image_send_columns/321455161d79e2c45d6f5a6f175d6f765f418897/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/image_send_columns/321455161d79e2c45d6f5a6f175d6f765f418897/1200w.png">
      <img src="https://static.rerun.io/image_send_columns/321455161d79e2c45d6f5a6f175d6f765f418897/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in image_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            buffer=None,  # type: ignore[arg-type]
            format=None,  # type: ignore[arg-type]
            opacity=None,  # type: ignore[arg-type]
            draw_order=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Image:
        """Produce an empty Image, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    buffer: components.ImageBufferBatch = field(
        metadata={"component": "required"},
        converter=components.ImageBufferBatch._required,  # type: ignore[misc]
    )
    # The raw image data.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    format: components.ImageFormatBatch = field(
        metadata={"component": "required"},
        converter=components.ImageFormatBatch._required,  # type: ignore[misc]
    )
    # The format of the image.
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
