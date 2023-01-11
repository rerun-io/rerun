import os
from enum import Enum
from typing import Final, Optional, Sequence, Union

import numpy as np
import numpy.typing as npt
from rerun.color_conversion import linear_to_gamma_u8_pixel

from rerun import bindings

__all__ = [
    "ArrowState",
    "EXP_ARROW",
    "annotation",
    "arrow",
    "bounding_box",
    "camera",
    "error_utils",
    "file",
    "image",
    "lines",
    "points",
    "rects",
    "scalar",
    "tensor",
    "text",
    "transform",
]


class ArrowState(Enum):
    """ArrowState is a enum used to configure the logging behaviour of the SDK during the transition to Arrow."""

    # No Arrow loggin
    NONE = "none"
    # Log both classic and Arrow
    MIXED = "mixed"
    # Log *only* Arrow
    PURE = "pure"

    def classic_log_gate(self) -> bool:
        """Should classic logging be performed."""
        return self in [ArrowState.NONE, ArrowState.MIXED]

    def arrow_log_gate(self) -> bool:
        """Should Arrow logging be performed."""
        return self in [ArrowState.MIXED, ArrowState.PURE]


try:
    env_var = os.environ.get("RERUN_EXP_ARROW")
    EXP_ARROW: Final = ArrowState[env_var.upper()] if env_var else ArrowState.NONE

except KeyError:
    raise RuntimeWarning(
        f"RERUN_EXP_ARROW should be set to one of {list(ArrowState.__members__.keys())}, but got {env_var}"
    )


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


def log_cleared(obj_path: str, *, recursive: bool = False) -> None:
    """
    Indicate that an object at a given path should no longer be displayed.

    If `recursive` is True this will also clear all sub-paths
    """
    if EXP_ARROW.classic_log_gate():
        bindings.log_cleared(obj_path, recursive)

    if EXP_ARROW.arrow_log_gate():
        bindings.log_cleared("arrow/" + obj_path, recursive)


def set_visible(obj_path: str, visibile: bool) -> None:
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
