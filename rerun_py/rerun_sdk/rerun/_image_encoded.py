"""
Deprecated helpers.

Use `Image` and `EncodedImage` instead.
"""

from __future__ import annotations

import io
import pathlib
import warnings
from typing import IO

from .archetypes import EncodedImage, Image
from .datatypes import Float32Like


class ImageFormat:
    """⚠️ DEPRECATED ⚠️ Image file format."""

    name: str

    BMP: ImageFormat
    """
    BMP file format.
    """

    GIF: ImageFormat
    """
    JPEG/JPG file format.
    """

    JPEG: ImageFormat
    """
    JPEG/JPG file format.
    """

    PNG: ImageFormat
    """
    PNG file format.
    """

    TIFF: ImageFormat
    """
    TIFF file format.
    """

    NV12: type[NV12]
    """
    Raw NV12 encoded image.

    The type comes with a `size_hint` attribute, a tuple of (height, width)
    which has to be specified specifying in order to set the RGB size of the image.
    """

    YUY2: type[YUY2]
    """
    Raw YUY2 encoded image.

    YUY2 is a YUV422 encoding with bytes ordered as `yuyv`.

    The type comes with a `size_hint` attribute, a tuple of (height, width)
    which has to be specified specifying in order to set the RGB size of the image.
    """

    def __init__(self, name: str):
        self.name = name

    def __str__(self) -> str:
        return self.name


class NV12(ImageFormat):
    """⚠️ DEPRECATED ⚠️ NV12 format."""

    name = "NV12"
    size_hint: tuple[int, int]

    def __init__(self, size_hint: tuple[int, int]) -> None:
        """
        An NV12 encoded image.

        Parameters
        ----------
        size_hint:
            A tuple of (height, width), specifying the RGB size of the image

        """
        self.size_hint = size_hint


class YUY2(ImageFormat):
    """⚠️ DEPRECATED ⚠️ YUY2 format."""

    name = "YUY2"
    size_hint: tuple[int, int]

    def __init__(self, size_hint: tuple[int, int]) -> None:
        """
        An YUY2 encoded image.

        YUY2 is a YUV422 encoding with bytes ordered as `yuyv`.

        Parameters
        ----------
        size_hint:
            A tuple of (height, width), specifying the RGB size of the image

        """
        self.size_hint = size_hint


# Assign the variants
# This allows for rust like enums, for example:
# ImageFormat.NV12(width=1920, height=1080)
# isinstance(ImageFormat.NV12, ImageFormat) == True and isinstance(ImageFormat.NV12, NV12) == True
ImageFormat.BMP = ImageFormat("BMP")
ImageFormat.GIF = ImageFormat("GIF")
ImageFormat.JPEG = ImageFormat("JPEG")
ImageFormat.PNG = ImageFormat("PNG")
ImageFormat.TIFF = ImageFormat("TIFF")
ImageFormat.NV12 = NV12
ImageFormat.YUY2 = YUY2


def ImageEncoded(
    *,
    path: str | pathlib.Path | None = None,
    contents: bytes | IO[bytes] | None = None,
    format: ImageFormat | None = None,
    draw_order: Float32Like | None = None,
) -> Image | EncodedImage:
    """
    ⚠️ DEPRECATED ⚠️ - Use [`Image`][rerun.archetypes.Image] (NV12, YUYV, …) and [`EncodedImage`][rerun.archetypes.EncodedImage] (PNG, JPEG, …) instead.

    A monochrome or color image encoded with a common format (PNG, JPEG, etc.).

    The encoded image can be loaded from either a file using its `path` or
    provided directly via `contents`.

    Parameters
    ----------
    path:
        A path to a file stored on the local filesystem. Mutually
        exclusive with `contents`.
    contents:
        The contents of the file. Can be a BufferedReader, BytesIO, or
        bytes. Mutually exclusive with `path`.
    format:
        The format of the image file or image encoding.
        If not provided, it will be inferred from the file extension if a path is specified.
        Note that encodings like NV12 and YUY2 can not be inferred from the file extension.
    draw_order:
        An optional floating point value that specifies the 2D drawing
        order. Objects with higher values are drawn on top of those with
        lower values.

    """

    warnings.warn(
        message=(
            "`ImageEncoded` is deprecated. Use `Image` (for NV12 and YUY2) or `EncodedImage` (for PNG, JPEG, …) instead."
        ),
        category=DeprecationWarning,
    )

    if (path is None) == (contents is None):
        raise ValueError("Must provide exactly one of 'path' or 'contents'")

    if format is not None:
        if isinstance(format, NV12) or isinstance(format, YUY2):
            buffer: IO[bytes] | None
            if path is not None:
                buffer = io.BytesIO(pathlib.Path(path).read_bytes())
            elif isinstance(contents, bytes):
                buffer = io.BytesIO(contents)
            else:
                assert (
                    # For the type-checker - we've already ensured that either `path` or `contents` must be set
                    contents is not None
                )
                buffer = contents

            contentx_bytes = buffer.read()

            if isinstance(format, NV12):
                return Image(
                    bytes=contentx_bytes,
                    width=format.size_hint[1],
                    height=format.size_hint[0],
                    pixel_format="NV12",
                    draw_order=draw_order,
                )
            elif isinstance(format, YUY2):
                return Image(
                    bytes=contentx_bytes,
                    width=format.size_hint[1],
                    height=format.size_hint[0],
                    pixel_format="YUY2",
                    draw_order=draw_order,
                )

    media_type = None
    if format is not None:
        if str(format).upper() == "BMP":
            media_type = "image/bmp"
        elif str(format).upper() == "GIF":
            media_type = "image/gif"
        elif str(format).upper() == "JPEG":
            media_type = "image/jpeg"
        elif str(format).upper() == "PNG":
            media_type = "image/png"
        elif str(format).upper() == "TIFF":
            media_type = "image/tiff"
        else:
            raise ValueError(f"Unknown image format: {format}")

    if path is not None:
        return EncodedImage(
            path=path,
            media_type=media_type,
            draw_order=draw_order,
        )
    else:
        return EncodedImage(
            contents=contents,
            media_type=media_type,
            draw_order=draw_order,
        )
