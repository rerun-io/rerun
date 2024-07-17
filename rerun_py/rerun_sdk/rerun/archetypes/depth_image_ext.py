from __future__ import annotations

from typing import TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt

from ..components import Colormap, ChannelDataType, Resolution2D

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
        data: ImageLike,
        *,
        meter: float | None = None,
        colormap: Colormap | None = None,
    ):
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

        data = _to_numpy(data)

        shape = data.shape

        # Ignore leading and trailing dimensions of size 1:
        while 2 < len(shape) and shape[0] == 1:
            shape = shape[1:]
        while 2 < len(shape) and shape[-1] == 1:
            shape = shape[:-1]

        if len(shape) != 2:
            raise ValueError(f"DepthImage must be 2D, got shape {data.shape}")
        height, width = shape

        try:
            data_type = channel_dtype_from_np_dtype[data.dtype.type]
        except KeyError:
            raise ValueError(f"Unsupported dtype {data.dtype} for DepthImage")

        self.__attrs_init__(
            data=data.tobytes(),
            resolution=Resolution2D(width=width, height=height),
            data_type=data_type,
            meter=meter,
            colormap=colormap,
        )
