from __future__ import annotations

from typing import TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt

from ..components import Colormap, ImageFormat
from ..datatypes import ChannelDatatype, Float32Like

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


class DepthImageExt:
    """Extension for [DepthImage][rerun.archetypes.DepthImage]."""

    def __init__(
        self: Any,
        image: ImageLike,
        *,
        meter: Float32Like | None = None,
        colormap: Colormap | None = None,
    ):
        image = _to_numpy(image)

        shape = image.shape

        # Ignore leading and trailing dimensions of size 1:
        while 2 < len(shape) and shape[0] == 1:
            shape = shape[1:]
        while 2 < len(shape) and shape[-1] == 1:
            shape = shape[:-1]

        if len(shape) != 2:
            raise ValueError(f"DepthImage must be 2D, got shape {image.shape}")
        height, width = shape

        try:
            datatype = ChannelDatatype.from_np_dtype(image.dtype)
        except KeyError:
            raise ValueError(f"Unsupported dtype {image.dtype} for DepthImage")

        self.__attrs_init__(
            buffer=image.tobytes(),
            format=ImageFormat(
                width=width,
                height=height,
                channel_datatype=datatype,
            ),
            meter=meter,
            colormap=colormap,
        )
