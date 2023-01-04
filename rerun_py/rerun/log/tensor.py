import logging
from typing import Any, Iterable, Optional, Union

import numpy as np
import numpy.typing as npt
from rerun.log import EXP_ARROW

from rerun import bindings

__all__ = [
    "log_tensor",
]

TensorDType = Union[
    np.uint8, np.uint16, np.uint32, np.uint64, np.int8, np.int16, np.int32, np.int64, np.float16, np.float32, np.float64
]


def log_tensor(
    obj_path: str,
    tensor: npt.NDArray[TensorDType],
    names: Optional[Iterable[str]] = None,
    meter: Optional[float] = None,
    timeless: bool = False,
) -> None:
    _log_tensor(obj_path, tensor=tensor, names=names, meter=meter, timeless=timeless)


def _log_tensor(
    obj_path: str,
    tensor: npt.NDArray[TensorDType],
    names: Optional[Iterable[str]] = None,
    meter: Optional[float] = None,
    meaning: bindings.TensorDataMeaning = None,
    timeless: bool = False,
) -> None:
    """Log a general tensor, perhaps with named dimensions."""
    if names is not None:
        names = list(names)
        assert len(tensor.shape) == len(names)

    SUPPORTED_DTYPES: Any = [
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
    ]

    if tensor.dtype not in SUPPORTED_DTYPES:
        raise TypeError(f"Unsupported dtype: {tensor.dtype}. Expected a numeric type.")

    if EXP_ARROW.classic_log_gate():
        bindings.log_tensor(obj_path, tensor, names, meter, meaning, timeless)

    if EXP_ARROW.arrow_log_gate():
        logging.warning("log_tensor() not yet implemented for Arrow.")
