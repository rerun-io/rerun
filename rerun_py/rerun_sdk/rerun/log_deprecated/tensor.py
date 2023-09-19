from __future__ import annotations

from typing import Any, Protocol, Sequence, Union

import numpy as np
import numpy.typing as npt

from rerun.datatypes.tensor_data import TensorDataLike
from rerun.error_utils import _send_warning
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_tensor",
]


class TorchTensorLike(Protocol):
    """Describes what is need from a Torch Tensor to be loggable to Rerun."""

    def numpy(self, force: bool) -> npt.NDArray[Any]:
        ...


Tensor = Union[npt.ArrayLike, TorchTensorLike]
"""Type helper for a tensor-like object that can be logged to Rerun."""


def _to_numpy(tensor: Tensor) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)  # type: ignore[union-attr]
    except AttributeError:
        return np.array(tensor, copy=False)


@log_decorator
def log_tensor(
    entity_path: str,
    tensor: TensorDataLike,
    *,
    names: Sequence[str | None] | None = None,
    meter: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log an n-dimensional tensor.

    Parameters
    ----------
    entity_path:
        Path to the tensor in the space hierarchy.
    tensor:
        A [Tensor][rerun.log.tensor.Tensor] object.
    names:
        Optional names for each dimension of the tensor.
    meter:
        Optional scale of the tensor (e.g. meters per cell).
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the tensor will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    if meter is not None:
        _send_warning("The `meter` argument is deprecated for use with `log_tensor`. Use `log_depth_image` instead.", 1)

    _log_tensor(
        entity_path,
        tensor=tensor,
        names=names,
        ext=ext,
        timeless=timeless,
        recording=recording,
    )


def _log_tensor(
    entity_path: str,
    tensor: TensorDataLike,
    names: Sequence[str | None] | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """Log a general tensor, perhaps with named dimensions."""
    from rerun.experimental import Tensor, dt, log

    tensor_data = dt.TensorData(array=tensor, names=names)

    log(entity_path, Tensor(tensor_data), ext=ext, timeless=timeless, recording=recording)
