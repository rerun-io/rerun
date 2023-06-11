from __future__ import annotations

from typing import Any, Iterable, Protocol, Union

import numpy as np
import numpy.typing as npt

from rerun import bindings
from rerun.components.draw_order import DrawOrderArray
from rerun.components.instance import InstanceArray
from rerun.components.tensor import TensorArray
from rerun.log.error_utils import _send_warning
from rerun.log.extension_components import _add_extension_components
from rerun.log.log_decorator import log_decorator
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
    tensor: npt.ArrayLike,
    *,
    names: Iterable[str | None] | None = None,
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
    _log_tensor(
        entity_path,
        tensor=_to_numpy(tensor),
        names=names,
        meter=meter,
        ext=ext,
        timeless=timeless,
        recording=recording,
    )


def _log_tensor(
    entity_path: str,
    tensor: npt.NDArray[Any],
    draw_order: float | None = None,
    names: Iterable[str | None] | None = None,
    meter: float | None = None,
    meaning: bindings.TensorDataMeaning = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
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
                recording=recording,
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

    if tensor.dtype not in SUPPORTED_DTYPES:
        _send_warning(
            f"Unsupported dtype: {tensor.dtype}. Expected a numeric type. Skipping this tensor.",
            2,
            recording=recording,
        )
        return

    instanced: dict[str, Any] = {}
    splats: dict[str, Any] = {}

    instanced["rerun.tensor"] = TensorArray.from_numpy(tensor, names, meaning, meter)

    if draw_order is not None:
        instanced["rerun.draw_order"] = DrawOrderArray.splat(draw_order)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(
            entity_path,
            components=splats,
            timeless=timeless,
            recording=recording,
        )

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(
            entity_path,
            components=instanced,
            timeless=timeless,
            recording=recording,
        )
