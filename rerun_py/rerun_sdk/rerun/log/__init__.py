from typing import Optional, Sequence, Union

import numpy as np
import numpy.typing as npt
from rerun.color_conversion import linear_to_gamma_u8_pixel

from rerun import bindings

__all__ = [
    "annotation",
    "arrow",
    "bounding_box",
    "camera",
    "error_utils",
    "file",
    "image",
    "lines",
    "mesh",
    "points",
    "rects",
    "scalar",
    "tensor",
    "text",
    "transform",
    "ext",
]


ColorDtype = Union[np.uint8, np.float32, np.float64]
Colors = npt.NDArray[ColorDtype]
Color = Union[npt.NDArray[ColorDtype], Sequence[Union[int, float]]]

OptionalClassIds = Optional[Union[int, npt.ArrayLike]]
OptionalKeyPointIds = Optional[Union[int, npt.ArrayLike]]


def _to_sequence(array: Optional[npt.ArrayLike]) -> Optional[Sequence[float]]:
    if isinstance(array, np.ndarray):
        return np.require(array, float).tolist()  # type: ignore[no-any-return]

    return array  # type: ignore[return-value]


def _normalize_colors(colors: Optional[npt.ArrayLike] = None) -> npt.NDArray[np.uint8]:
    """Normalize flexible colors arrays."""
    if colors is None:
        # An empty array represents no colors.
        return np.array((), dtype=np.uint8)
    else:
        colors_array = np.array(colors, copy=False)

        # Rust expects colors in 0-255 uint8
        if colors_array.dtype.type in [np.float32, np.float64]:
            return linear_to_gamma_u8_pixel(linear=colors_array)

        return np.require(colors_array, np.uint8)


def _normalize_ids(class_ids: OptionalClassIds = None) -> npt.NDArray[np.uint16]:
    """Normalize flexible class id arrays."""
    if class_ids is None:
        return np.array((), dtype=np.uint16)
    else:
        # TODO(andreas): Does this need optimizing for the case where class_ids is already an np array?
        return np.atleast_1d(np.array(class_ids, dtype=np.uint16, copy=False))


def _normalize_radii(radii: Optional[npt.ArrayLike] = None) -> npt.NDArray[np.float32]:
    """Normalize flexible radii arrays."""
    if radii is None:
        return np.array((), dtype=np.float32)
    else:
        return np.atleast_1d(np.array(radii, dtype=np.float32, copy=False))


def _normalize_labels(labels: Optional[Union[str, Sequence[str]]]) -> Sequence[str]:
    if labels is None:
        return []
    else:
        return labels


def log_cleared(entity_path: str, *, recursive: bool = False) -> None:
    """
    Indicate that an entity at a given path should no longer be displayed.

    If `recursive` is True this will also clear all sub-paths
    """
    bindings.log_cleared(entity_path, recursive)


def set_visible(entity_path: str, visibile: bool) -> None:
    """
    set_visible has been deprecated.

    The replacement is `log_cleared()`.
    See: https://github.com/rerun-io/rerun/pull/285 for details
    """
    # This is a slight abose of DeprecationWarning compared to using
    # warning.warn, but there is no function to call here anymore.
    # this is (slightly) better than just failing on an undefined function
    # TODO(jleibs) Remove after 11/25
    raise DeprecationWarning("set_visible has been deprecated. please use log_cleared")
