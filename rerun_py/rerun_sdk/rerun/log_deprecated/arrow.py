from __future__ import annotations

from typing import Any

import numpy.typing as npt
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.any_value import AnyValues
from rerun.archetypes import Arrows3D
from rerun.log_deprecated import Color
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_arrow",
]


@deprecated(
    """Please migrate to `rr.log(…, rr.Arrows3D(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_arrow(
    entity_path: str,
    origin: npt.ArrayLike,
    vector: npt.ArrayLike,
    *,
    color: Color | None = None,
    label: str | None = None,
    width_scale: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a 3D arrow.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Arrows3D][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

    An arrow is defined with an `origin`, and a `vector`. This can also be considered as `start` and `end` positions
    for the arrow.

    The shaft is rendered as a cylinder with `radius = 0.5 * width_scale`.
    The tip is rendered as a cone with `height = 2.0 * width_scale` and `radius = 1.0 * width_scale`.

    Parameters
    ----------
    entity_path
        The path to store the entity at.
    origin
        The base position of the arrow.
    vector
        The vector along which the arrow will be drawn.
    color
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    label
        An optional text to show beside the arrow.
    width_scale
        An optional scaling factor, default=1.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless
        The entity is not time-dependent, and will be visible at any time point.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    arrows3d = Arrows3D(
        vectors=vector,
        origins=origin,
        radii=width_scale * 0.5 if width_scale is not None else None,
        colors=color,
        labels=label,
    )
    return log(entity_path, arrows3d, AnyValues(**(ext or {})), timeless=timeless, recording=recording)
