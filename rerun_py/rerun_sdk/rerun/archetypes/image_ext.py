from __future__ import annotations

from io import BytesIO
from typing import TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt

from ..components import ImageFormat
from ..datatypes import (
    ChannelDatatype,
    ChannelDatatypeLike,
    ColorModel,
    ColorModelLike,
    Float32Like,
    PixelFormatLike,
)
from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    ImageLike = Union[
        npt.NDArray[np.float16],
        npt.NDArray[np.float32],
        npt.NDArray[np.float64],
        npt.NDArray[np.int16],
        npt.NDArray[np.int32],
        npt.NDArray[np.int64],
        npt.NDArray[np.int8],
        npt.NDArray[np.uint16],
        npt.NDArray[np.uint32],
        npt.NDArray[np.uint64],
        npt.NDArray[np.uint8],
    ]
    from . import EncodedImage, Image


def _to_numpy(tensor: ImageLike) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)  # type: ignore[union-attr]
    except AttributeError:
        return np.array(tensor, copy=False)


class ImageExt:
    """Extension for [Image][rerun.archetypes.Image]."""

    def __init__(
        self: Any,
        # These:
        image: ImageLike | None = None,
        color_model: ColorModelLike | None = None,
        *,
        # Or these:
        pixel_format: PixelFormatLike | None = None,
        datatype: ChannelDatatypeLike | type | None = None,
        bytes: bytes | None = None,
        width: int | None = None,
        height: int | None = None,
        # Any any of these:
        opacity: Float32Like | None = None,
        draw_order: Float32Like | None = None,
    ):
        """
        Create a new image with a given format.

        There are three ways to create an image:
        * By specifying an `image` as an appropriately shaped ndarray with an appropriate `color_model`.
        * By specifying `bytes` of an image with a `pixel_format`, together with `width`, `height`.
        * By specifying `bytes` of an image with a `datatype` and `color_model`, together with `width`, `height`.

        Parameters
        ----------
        image:
            A numpy array or tensor with the image data.
            Leading and trailing unit-dimensions are ignored, so that
            `1x480x640x3x1` is treated as a `480x640x3`.
            You also need to specify the `color_model` of it (e.g. "RGB").
        color_model:
            L, RGB, RGBA, BGR, BGRA, etc, specifying how to interpret `image`.
        pixel_format:
            NV12, YUV420, etc. For chroma-downsampling.
            Requires `width`, `height`, and `bytes`.
        datatype:
            The datatype of the image data. If not specified, it is inferred from the `image`.
        bytes:
            The raw bytes of an image specified by `pixel_format`.
        width:
            The width of the image. Only requires for `pixel_format`.
        height:
            The height of the image. Only requires for `pixel_format`.
        opacity:
            Optional opacity of the image, in 0-1. Set to 0.5 for a translucent image.
        draw_order:
            An optional floating point value that specifies the 2D drawing
            order. Objects with higher values are drawn on top of those with
            lower values.

        """

        channel_count_from_color_model = {
            "a": 1,
            "l": 1,
            "la": 1,
            "bgr": 3,
            "rgb": 3,
            "yuv": 3,
            "bgra": 4,
            "rgba": 4,
        }

        # If the user specified 'bytes', we can use direct construction
        if bytes is not None:
            if isinstance(bytes, np.ndarray):
                bytes = bytes.tobytes()

            if width is None or height is None or bytes is None:
                raise ValueError("Specifying 'bytes' requires 'width' and 'height'")

            if pixel_format is not None:
                if datatype is not None:
                    raise ValueError("Specifying 'datatype' is mutually exclusive with 'pixel_format'")
                if color_model is not None:
                    raise ValueError("Specifying 'color_model' is mutually exclusive with 'pixel_format'")

                # TODO(jleibs): Validate that bytes is the expected size.

                self.__attrs_init__(
                    buffer=bytes,
                    format=ImageFormat(width=width, height=height, pixel_format=pixel_format),
                    opacity=opacity,
                    draw_order=draw_order,
                )
                return
            else:
                if datatype is None or color_model is None:
                    raise ValueError("Specifying 'bytes' requires 'pixel_format' or both 'color_model' and 'datatype'")

                # TODO(jleibs): Would be nice to do this with a field-converter
                if datatype in (
                    np.uint8,
                    np.uint16,
                    np.uint32,
                    np.uint64,
                    np.int8,
                    np.int16,
                    np.int32,
                    np.int64,
                    np.float16,
                    np.float32,
                    np.float64,
                ):
                    datatype = ChannelDatatype.from_np_dtype(np.dtype(datatype))  # type: ignore[arg-type]

                # TODO(jleibs): Validate that bytes is the expected size.

                self.__attrs_init__(
                    buffer=bytes,
                    format=ImageFormat(
                        width=width,
                        height=height,
                        channel_datatype=datatype,  # type: ignore[arg-type]
                        color_model=color_model,
                    ),
                    opacity=opacity,
                    draw_order=draw_order,
                )
                return

        # Alternatively, we extract the values from the image-like
        if image is None:
            raise ValueError("Must specify either 'image' or 'bytes'")

        image = _to_numpy(image)

        shape = image.shape

        # Ignore leading and trailing dimensions of size 1:
        while 2 < len(shape) and shape[0] == 1:
            shape = shape[1:]
        while 2 < len(shape) and shape[-1] == 1:
            shape = shape[:-1]

        if len(shape) == 2:
            _height, _width = shape
            channels = 1
        elif len(shape) == 3:
            _height, _width, channels = shape
        else:
            raise ValueError(f"Expected a 2D or 3D tensor, got {shape}")

        if width is not None and width != _width:
            raise ValueError(f"Provided width {width} does not match image width {_width}")
        else:
            width = _width

        if height is not None and height != _height:
            raise ValueError(f"Provided height {height} does not match image height {_height}")
        else:
            height = _height

        if color_model is None:
            if channels == 1:
                color_model = ColorModel.L
            elif channels == 3:
                color_model = ColorModel.RGB
            elif channels == 4:
                color_model = ColorModel.RGBA
            else:
                _send_warning_or_raise(f"Expected 1, 3, or 4 channels; got {channels}")
        else:
            try:
                num_expected_channels = channel_count_from_color_model[str(color_model).lower()]
                if channels != num_expected_channels:
                    _send_warning_or_raise(
                        f"Expected {num_expected_channels} channels for {color_model}; got {channels} channels"
                    )
            except KeyError:
                _send_warning_or_raise(f"Unknown ColorModel: '{color_model}'")

        try:
            datatype = ChannelDatatype.from_np_dtype(image.dtype)
        except KeyError:
            _send_warning_or_raise(f"Unsupported dtype {image.dtype} for Image")

        self.__attrs_init__(
            buffer=image.tobytes(),
            format=ImageFormat(
                width=width,
                height=height,
                channel_datatype=datatype,  # type: ignore[arg-type]
                color_model=color_model,
            ),
            opacity=opacity,
            draw_order=draw_order,
        )

    def compress(self: Any, jpeg_quality: int = 95) -> EncodedImage | Image:
        """
        Compress the given image as a JPEG.

        JPEG compression works best for photographs.
        Only U8 RGB and grayscale images are supported, not RGBA.
        Note that compressing to JPEG costs a bit of CPU time,
        both when logging and later when viewing them.

        Parameters
        ----------
        jpeg_quality:
            Higher quality = larger file size.
            A quality of 95 saves a lot of space, but is still visually very similar.

        """

        from PIL import Image as PILImage

        from ..archetypes import EncodedImage

        with catch_and_log_exceptions(context="Image compression"):
            if self.format is None:
                raise ValueError("Cannot JPEG compress an image without a known image_format")

            image_format_arrow = self.format.as_arrow_array().storage[0].as_py()

            image_format = ImageFormat(
                width=image_format_arrow["width"],
                height=image_format_arrow["height"],
                pixel_format=image_format_arrow["pixel_format"],
                channel_datatype=image_format_arrow["channel_datatype"],
                color_model=image_format_arrow["color_model"],
            )

            # TODO(jleibs): Support conversions here
            if image_format.pixel_format is not None:
                raise ValueError(f"Cannot JPEG compress an image with pixel_format {image_format.pixel_format}")

            if image_format.color_model not in (ColorModel.L, ColorModel.RGB, ColorModel.BGR):
                raise ValueError(
                    f"Cannot JPEG compress an image of type {image_format.color_model}. Only L (monochrome), RGB and BGR are supported."
                )

            if image_format.channel_datatype != ChannelDatatype.U8:
                # See: https://pillow.readthedocs.io/en/stable/handbook/concepts.html#concept-modes
                # Note that modes F and I do not support jpeg compression
                raise ValueError(
                    f"Cannot JPEG compress an image of datatype {image_format.channel_datatype}. Only U8 is supported."
                )

            buf = None
            if self.buffer is not None:
                buf = (
                    self.buffer.as_arrow_array()
                    .storage.values.to_numpy()
                    .view(image_format.channel_datatype.to_np_dtype())
                )

            if buf is None:
                raise ValueError("Cannot JPEG compress an image without data")

            # Note: np array shape is always (height, width, channels)
            if image_format.color_model == ColorModel.L:
                image = buf.reshape(image_format.height, image_format.width)
            else:
                image = buf.reshape(image_format.height, image_format.width, 3)

            # PIL doesn't understand BGR.
            if image_format.color_model == ColorModel.BGR:
                mode = "RGB"
                image = image[:, :, ::-1]
            else:
                mode = str(image_format.color_model)

            pil_image = PILImage.fromarray(image, mode=mode)
            output = BytesIO()
            pil_image.save(output, format="JPEG", quality=jpeg_quality)
            jpeg_bytes = output.getvalue()
            output.close()
            return EncodedImage(contents=jpeg_bytes, media_type="image/jpeg")

        # On failure to compress, return a raw image
        return self  # type: ignore[no-any-return]
