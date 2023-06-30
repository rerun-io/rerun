from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt

from rerun import bindings
from rerun.components.arrow import Arrow3DArray
from rerun.components.color import ColorRGBAArray
from rerun.components.instance import InstanceArray
from rerun.components.label import LabelArray
from rerun.components.radius import RadiusArray
from rerun.log import Color, _normalize_colors, _normalize_radii
from rerun.log.extension_components import _add_extension_components
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_arrow",
]


@log_decorator
def log_arrow(
    entity_path: str,
    origin: npt.ArrayLike | None,
    vector: npt.ArrayLike | None = None,
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

    instanced: dict[str, Any] = {}
    splats: dict[str, Any] = {}

    if origin is not None:
        if vector is None:
            raise TypeError("Must provide both origin and vector")
        origin = np.require(origin, dtype="float32")
        vector = np.require(vector, dtype="float32")
        instanced["rerun.arrow3d"] = Arrow3DArray.from_numpy(origin.reshape(1, 3), vector.reshape(1, 3))

    if color is not None:
        colors = _normalize_colors(color)
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if label:
        instanced["rerun.label"] = LabelArray.new([label])

    if width_scale:
        radii = _normalize_radii([width_scale / 2])
        instanced["rerun.radius"] = RadiusArray.from_numpy(radii)

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
