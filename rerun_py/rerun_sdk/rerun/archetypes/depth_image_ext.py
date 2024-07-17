from __future__ import annotations

from typing import TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt

if TYPE_CHECKING:
    from ..components import Colormap, ElementType, Resolution2D

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
        pixels: ImageLike,
        *,
        meter: float | None = None,
        colormap: Colormap | None = None,
    ):
        element_type_from_dtype = {
            np.uint8: ElementType.U8,
            np.uint16: ElementType.U16,
            np.uint32: ElementType.U32,
            np.uint64: ElementType.U64,
            np.int8: ElementType.I8,
            np.int16: ElementType.I16,
            np.int32: ElementType.I32,
            np.int64: ElementType.I64,
            np.float16: ElementType.F16,
            np.float32: ElementType.F32,
            np.float64: ElementType.F64,
        }

        pixels = _to_numpy(pixels)

        shape = pixels.shape
        if len(shape) != 2:
            raise ValueError(f"DepthImage must be 2D, got shape {shape}")
        height, width = shape

        try:
            element_type = element_type_from_dtype[pixels.dtype.type]
        except KeyError:
            raise ValueError(f"Unsupported dtype {pixels.dtype} for DepthImage")

        self.__attrs_init__(
            data=pixels.tobytes(),
            resolution=Resolution2D(width=width, height=height),
            element_type=element_type,
            meter=meter,
            colormap=colormap,
        )
