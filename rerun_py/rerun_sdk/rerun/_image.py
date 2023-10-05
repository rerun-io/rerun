from __future__ import annotations

import io
import pathlib
from enum import Enum
from typing import IO, Iterable

import numpy as np
from PIL import Image as PILImage

from ._log import AsComponents, ComponentBatchLike
from .archetypes import Image
from .components import DrawOrderLike, TensorData
from .datatypes import TensorBuffer, TensorDimension

__all__ = ["ImageFormat", "ImageEncoded"]


class ImageFormat(Enum):
    """Image file format."""

    BMP = "BMP"
    """BMP format."""

    GIF = "GIF"
    """GIF format."""

    JPEG = "JPEG"
    """JPEG format."""

    PNG = "PNG"
    """PNG format."""

    TIFF = "TIFF"
    """TIFF format."""

    def __str__(self) -> str:
        return self.name


class ImageEncoded(AsComponents):
    """
    A monochrome or color image encoded with a common format (PNG, JPEG, etc.).

    The encoded image can be loaded from either a file using its `path` or
    provided directly via `contents`.
    """

    def __init__(
        self,
        *,
        path: str | pathlib.Path | None = None,
        contents: bytes | IO[bytes] | None = None,
        format: ImageFormat | None = None,
        draw_order: DrawOrderLike | None = None,
    ) -> None:
        """
        Create a new image with a given format.

        Parameters
        ----------
        path:
            A path to a file stored on the local filesystem. Mutually
            exclusive with `contents`.
        contents:
            The contents of the file. Can be a BufferedReader, BytesIO, or
            bytes. Mutually exclusive with `path`.
        format:
            The format of the image file. If not provided, it will be inferred
            from the file extension.
        draw_order:
            An optional floating point value that specifies the 2D drawing
            order. Objects with higher values are drawn on top of those with
            lower values.
        """
        if (path is None) == (contents is None):
            raise ValueError("Must provide exactly one of 'path' or 'contents'")

        buffer: IO[bytes] | None = None
        if path is not None:
            buffer = open(path, "rb")
        elif isinstance(contents, bytes):
            buffer = io.BytesIO(contents)
        else:
            buffer = contents

        if buffer is None:
            raise ValueError("Input data could not be coerced to IO[bytes]")

        if format is not None:
            formats = (str(format),)
        else:
            formats = None

        # Note that PIL loading is lazy. This will only identify the type of file
        # and not decode the whole jpeg.
        img_data = PILImage.open(buffer, formats=formats)

        if img_data.format == "JPEG":
            buffer.seek(0)
            np_buffer = buffer.read()
            tensor_buffer = TensorBuffer(np.frombuffer(np_buffer, dtype=np.uint8))
            tensor_buffer.kind = "jpeg"

            tensor_shape = (
                TensorDimension(img_data.height, "height"),
                TensorDimension(img_data.width, "width"),
                TensorDimension(3, "depth"),
            )
            tensor_data = TensorData(buffer=tensor_buffer, shape=tensor_shape)
        else:
            tensor_data = TensorData(array=np.asarray(img_data))

        self.data = tensor_data
        self.draw_order = draw_order

    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        return Image(self.data, draw_order=self.draw_order).as_component_batches()
