from __future__ import annotations

from typing import TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt

from rerun.components.channel_datatype import ChannelDatatype, ChannelDatatypeLike
from rerun.components.color_model import ColorModel, ColorModelLike
from rerun.components.pixel_format import PixelFormatLike

from ..components import Resolution2D
from ..datatypes import Float32Like
from ..error_utils import _send_warning_or_raise

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
            L, RGB, RGBA, etc, specifying how to interpret `image`.
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
                    data=bytes,
                    resolution=Resolution2D(width=width, height=height),
                    pixel_format=pixel_format,
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
                    data=bytes,
                    resolution=Resolution2D(width=width, height=height),
                    color_model=color_model,
                    datatype=datatype,
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
            datatype = ChannelDatatype.from_np_dtype(image.dtype)
        except KeyError:
            _send_warning_or_raise(f"Unsupported dtype {image.dtype} for Image")

        self.__attrs_init__(
            data=image.tobytes(),
            resolution=Resolution2D(width=width, height=height),
            color_model=color_model,
            datatype=datatype,
            opacity=opacity,
            draw_order=draw_order,
        )
