from typing import Any, Iterable, Optional, Protocol, Union

import numpy as np
import numpy.typing as npt
from rerun.components.tensor import TensorArray
from rerun.log.error_utils import _send_warning

from rerun import bindings

__all__ = [
    "log_tensor",
]


class TorchTensorLike(Protocol):
    """Describes what is need from a Torch Tensor to be loggable to Rerun."""

    def numpy(self, force: bool) -> npt.NDArray[Any]:
        ...


Tensor = Union[npt.ArrayLike, TorchTensorLike]


def _to_numpy(tensor: Tensor) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)  # type: ignore[union-attr]
    except AttributeError:
        return np.array(tensor, copy=False)


def log_tensor(
    entity_path: str,
    tensor: npt.ArrayLike,
    names: Optional[Iterable[str]] = None,
    meter: Optional[float] = None,
    timeless: bool = False,
) -> None:
    _log_tensor(entity_path, tensor=_to_numpy(tensor), names=names, meter=meter, timeless=timeless)


def _log_tensor(
    entity_path: str,
    tensor: npt.NDArray[Any],
    names: Optional[Iterable[Optional[str]]] = None,
    meter: Optional[float] = None,
    meaning: bindings.TensorDataMeaning = None,
    timeless: bool = False,
) -> None:
    """Log a general tensor, perhaps with named dimensions."""
    if names is not None:
        names = list(names)

        if len(tensor.shape) != len(names):
            _send_warning(
                (
                    f"len(tensor.shape) = len({tensor.shape}) = {len(tensor.shape)} != "
                    + f"len(names) = len({names}) = {len(names)}. Dropping tensor dimension names."
                ),
                2,
            )
            names = None

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

    # We don't support float16 -- upscale to f32
    # TODO(#854): Native F16 support for arrow tensors
    if tensor.dtype == np.float16:
        tensor = np.asarray(tensor, dtype="float32")

    if tensor.dtype not in SUPPORTED_DTYPES:
        _send_warning(f"Unsupported dtype: {tensor.dtype}. Expected a numeric type. Skipping this tensor.", 2)
        return

    comps = {"rerun.tensor": TensorArray.from_numpy(tensor, names, meaning, meter)}

    bindings.log_arrow_msg(entity_path, components=comps, timeless=timeless)
