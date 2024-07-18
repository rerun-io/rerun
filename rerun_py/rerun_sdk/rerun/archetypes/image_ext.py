from __future__ import annotations

from io import BytesIO
from typing import TYPE_CHECKING, Any, Union, cast

import numpy as np
import numpy.typing as npt

from rerun.components.color_model import ColorModel, ColorModelLike
from rerun.components.pixel_format import PixelFormatLike

from .._validators import find_non_empty_dim_indices
from ..components import ChannelDataType, Resolution2D
from ..datatypes import Float32Like
from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    from ..archetypes import ImageEncoded
    from . import Image

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


def _to_numpy(tensor: ImageLike) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)  # type: ignore[union-attr]
    except AttributeError:
        return np.array(tensor, copy=False)


# TODO: add to constructor docs
# As such, the shape of the [components.TensorData] must be mappable to:
# - A `HxW` tensor, treated as a grayscale image.
# - A `HxWx3` tensor, treated as an RGB image.
# - A `HxWx4` tensor, treated as an RGBA image.
#
# Leading and trailing unit-dimensions are ignored, so that
# `1x480x640x3x1` is treated as a `480x640x3` RGB image.


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
        bytes: bytes | None = None,
        width: int | None = None,
        height: int | None = None,
        # Any any of these:
        opacity: Float32Like | None = None,
        draw_order: Float32Like | None = None,
    ):
        """
        Create a new image with a given format.

        There are two ways to create an image:
        * By specifying an `image` and a `color_model`.
        * By specifying a `pixel_format`, together with `width`, `height`, and `bytes`.

        Parameters
        ----------
        image:
            A numpy array or tensor with the data.
            You also need to specify the `color_model` of it (e.g. "RGB").
        color_model:
            L, RGB, RGBA, etc, specifying how to interpret `image`.
        pixel_format:
            NV12, YUV420, etc. For chroma-downsampling.
            Requires `width`, `height`, and `bytes`.
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

        channel_dtype_from_np_dtype = {
            np.uint8: ChannelDataType.U8,
            np.uint16: ChannelDataType.U16,
            np.uint32: ChannelDataType.U32,
            np.uint64: ChannelDataType.U64,
            np.int8: ChannelDataType.I8,
            np.int16: ChannelDataType.I16,
            np.int32: ChannelDataType.I32,
            np.int64: ChannelDataType.I64,
            np.float16: ChannelDataType.F16,
            np.float32: ChannelDataType.F32,
            np.float64: ChannelDataType.F64,
        }

        channel_count_from_color_model = {
            "l": 1,
            "rgb": 3,
            "rgba": 4,
        }

        if pixel_format is not None:
            if width is None or height is None or bytes is None:
                raise ValueError("Must provide 'width', 'height', and 'bytes' with 'pixel_format'")

            if isinstance(bytes, np.ndarray):
                bytes = bytes.tobytes()

            self.__attrs_init__(
                data=bytes,
                resolution=Resolution2D(width=width, height=height),
                pixel_format=pixel_format,
                opacity=opacity,
                draw_order=draw_order,
            )
            return

        if image is None:
            raise ValueError("Missing `image` argument")

        image = _to_numpy(image)

        shape = image.shape

        # Ignore leading and trailing dimensions of size 1:
        while 2 < len(shape) and shape[0] == 1:
            shape = shape[1:]
        while 2 < len(shape) and shape[-1] == 1:
            shape = shape[:-1]

        if len(shape) == 2:
            height, width = shape
            channels = 1
        elif len(shape) == 3:
            height, width, channels = shape
        else:
            raise ValueError(f"Expected a 2D or 3D tensor, got {shape}")

        if color_model is None:
            if channels == 1:
                color_model = ColorModel.L
            elif channels == 3:
                color_model = ColorModel.RGB  # TODO(#2340): change default to BGR
            elif channels == 4:
                color_model = ColorModel.RGBA  # TODO(#2340): change default to BGRA
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
            data_type = channel_dtype_from_np_dtype[image.dtype.type]
        except KeyError:
            _send_warning_or_raise(f"Unsupported dtype {image.dtype} for Image")

        self.__attrs_init__(
            data=image.tobytes(),
            resolution=Resolution2D(width=width, height=height),
            color_model=color_model,
            data_type=data_type,
            opacity=opacity,
            draw_order=draw_order,
        )

    def compress(self, *, jpeg_quality: int = 95) -> ImageEncoded | Image:
        """
        Converts an `Image` to an [`rerun.ImageEncoded`][] using JPEG compression.

        JPEG compression works best for photographs. Only RGB or Mono images are
        supported, not RGBA. Note that compressing to JPEG costs a bit of CPU time,
        both when logging and later when viewing them.

        Parameters
        ----------
        jpeg_quality:
            Higher quality = larger file size. A quality of 95 still saves a lot
            of space, but is visually very similar.

        """

        from PIL import Image as PILImage

        from ..archetypes import ImageEncoded
        from . import Image

        self = cast(Image, self)

        with catch_and_log_exceptions(context="Image compression"):
            tensor_data_arrow = self.data.as_arrow_array()

            shape_dims = tensor_data_arrow[0].value["shape"].values.field(0).to_numpy()
            non_empty_dims = find_non_empty_dim_indices(shape_dims)
            filtered_shape = shape_dims[non_empty_dims]
            if len(filtered_shape) == 2:
                mode = "L"
            elif len(filtered_shape) == 3 and filtered_shape[-1] == 3:
                mode = "RGB"
            else:
                raise ValueError("Only RGB or Mono images are supported for JPEG compression")

            image_array = tensor_data_arrow[0].value["buffer"].value.values.to_numpy().reshape(filtered_shape)

            if image_array.dtype not in ["uint8", "sint32", "float32"]:
                # Convert to a format supported by Image.fromarray
                image_array = image_array.astype("float32")

            pil_image = PILImage.fromarray(image_array, mode=mode)
            output = BytesIO()
            pil_image.save(output, format="JPEG", quality=jpeg_quality)
            jpeg_bytes = output.getvalue()
            output.close()
            return ImageEncoded(contents=jpeg_bytes, media_type="image/jpeg")

        # On failure to compress, still return the original image
        return self
