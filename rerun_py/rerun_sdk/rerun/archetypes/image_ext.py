from __future__ import annotations

from typing import TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt

from rerun.components.color_model import ColorModel, ColorModelLike
from rerun.components.pixel_format import PixelFormatLike

from ..components import ChannelDatatype, Resolution2D
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
            A numpy array or tensor with the image data.
            Leading and trailing unit-dimensions are ignored, so that
            `1x480x640x3x1` is treated as a `480x640x3`.
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
