from __future__ import annotations

from typing import Any, Protocol, Sequence

import numpy.typing as npt
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.any_value import AnyValues
from rerun.archetypes import BarChart, Tensor
from rerun.datatypes.tensor_data import TensorData, TensorDataLike
from rerun.error_utils import _send_warning_or_raise
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_tensor",
]


class TorchTensorLike(Protocol):
    """Describes what is need from a Torch Tensor to be loggable to Rerun."""

    def numpy(self, force: bool) -> npt.NDArray[Any]:
        ...


@deprecated(
    """Please migrate to `rr.log(…, rr.Tensor(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
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

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Tensor][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

    Parameters
    ----------
    entity_path:
        Path to the tensor in the space hierarchy.
    tensor:
        A [Tensor][rerun.log_deprecated.tensor.Tensor] object.
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
        _send_warning_or_raise(
            "The `meter` argument is deprecated for use with `log_tensor`. Use `log_depth_image` instead.", 1
        )

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

    tensor_data = TensorData(array=tensor, dim_names=names)

    # Our legacy documentation is that 1D tensors were interpreted as barcharts
    if len(tensor_data.shape) == 1:
        log(entity_path, BarChart(tensor_data), AnyValues(**(ext or {})), timeless=timeless, recording=recording)
    else:
        log(entity_path, Tensor(tensor_data), AnyValues(**(ext or {})), timeless=timeless, recording=recording)
