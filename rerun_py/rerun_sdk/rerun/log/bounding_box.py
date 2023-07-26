from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import numpy.typing as npt

from rerun import bindings
from rerun.components.annotation import ClassIdArray
from rerun.components.box import Box3DArray
from rerun.components.color import ColorRGBAArray
from rerun.components.instance import InstanceArray
from rerun.components.label import LabelArray
from rerun.components.quaternion import QuaternionArray
from rerun.components.radius import RadiusArray
from rerun.components.vec import Vec3DArray
from rerun.log import (
    Color,
    Colors,
    OptionalClassIds,
    _normalize_colors,
    _normalize_ids,
    _normalize_labels,
    _normalize_radii,
)
from rerun.log.extension_components import _add_extension_components
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_obb",
    "log_obbs",
]


@log_decorator
def log_obb(
    entity_path: str,
    *,
    half_size: npt.ArrayLike | None,
    position: npt.ArrayLike | None = None,
    rotation_q: npt.ArrayLike | None = None,
    color: Color | None = None,
    stroke_width: float | None = None,
    label: str | None = None,
    class_id: int | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a 3D Oriented Bounding Box, or OBB.

    Example:
    --------
    ```
    rr.log_obb("my_obb", half_size=[1.0, 2.0, 3.0], position=[0, 0, 0], rotation_q=[0, 0, 0, 1])
    ```

    Parameters
    ----------
    entity_path:
        The path to the oriented bounding box in the space hierarchy.
    half_size:
        Array with [x, y, z] half dimensions of the OBB.
    position:
        Optional array with [x, y, z] position of the OBB in world space.
    rotation_q:
        Optional array with quaternion coordinates [x, y, z, w] for the rotation from model to world space.
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    stroke_width:
        Optional width of the line edges.
    label:
        Optional text label placed at `position`.
    class_id:
        Optional class id for the OBB.  The class id provides colors and labels if not specified explicitly.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the bounding box will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)

    instanced: dict[str, Any] = {}
    splats: dict[str, Any] = {}

    if half_size is not None:
        half_size = np.require(half_size, dtype="float32")

        if half_size.shape[0] == 3:
            instanced["rerun.box3d"] = Box3DArray.from_numpy(half_size.reshape(1, 3))
        else:
            raise TypeError("half_size should be 1x3")

    if position is not None:
        position = np.require(position, dtype="float32")

        if position.shape[0] == 3:
            instanced["rerun.vec3d"] = Vec3DArray.from_numpy(position.reshape(1, 3))
        else:
            raise TypeError("position should be 1x3")

    if rotation_q is not None:
        rotation = np.require(rotation_q, dtype="float32")

        if rotation.shape[0] == 4:
            instanced["rerun.quaternion"] = QuaternionArray.from_numpy(rotation.reshape(1, 4))
        else:
            raise TypeError("rotation should be 1x4")

    if color is not None:
        colors = _normalize_colors(color)
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    # We store the stroke_width in radius
    if stroke_width:
        radii = _normalize_radii([stroke_width / 2])
        instanced["rerun.radius"] = RadiusArray.from_numpy(radii)

    if label:
        instanced["rerun.label"] = LabelArray.new([label])

    if class_id:
        class_ids = _normalize_ids([class_id])
        instanced["rerun.class_id"] = ClassIdArray.from_numpy(class_ids)

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
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)


@log_decorator
def log_obbs(
    entity_path: str,
    *,
    half_sizes: npt.ArrayLike | None,
    positions: npt.ArrayLike | None = None,
    rotations_q: npt.ArrayLike | None = None,
    colors: Color | Colors | None = None,
    stroke_widths: npt.ArrayLike | None = None,
    labels: Sequence[str] | None = None,
    class_ids: OptionalClassIds | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a 3D Oriented Bounding Box, or OBB.

    Example:
    --------
    ```
    rr.log_obb("my_obb", half_size=[1.0, 2.0, 3.0], position=[0, 0, 0], rotation_q=[0, 0, 0, 1])
    ```

    Parameters
    ----------
    entity_path:
        The path to the oriented bounding box in the space hierarchy.
    half_sizes:
        Nx3 Array. Each row is the [x, y, z] half dimensions of an OBB.
    positions:
        Optional Nx3 array. Each row is [x, y, z] positions of an OBB in world space.
    rotations_q:
        Optional Nx3 array. Each row is quaternion coordinates [x, y, z, w] for the rotation from model to world space.
    colors:
        Optional Nx3 or Nx4 array. Each row is RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers,
        with separate alpha.
    stroke_widths:
        Optional array of the width of the line edges.
    labels:
        Optional array of text labels placed at `position`.
    class_ids:
        Optional array of class id for the OBBs.  The class id provides colors and labels if not specified explicitly.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the bounding box will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)

    colors = _normalize_colors(colors)
    stroke_widths = _normalize_radii(stroke_widths)
    radii = stroke_widths / 2
    labels = _normalize_labels(labels)
    class_ids = _normalize_ids(class_ids)

    # 0 = instanced, 1 = splat
    comps = [{}, {}]  # type: ignore[var-annotated]

    if half_sizes is not None:
        half_sizes = np.require(half_sizes, dtype="float32")

        if len(half_sizes) == 0 or half_sizes.shape[1] == 3:
            comps[0]["rerun.box3d"] = Box3DArray.from_numpy(half_sizes)
        else:
            raise TypeError("half_size should be Nx3")

    if positions is not None:
        positions = np.require(positions, dtype="float32")

        if len(positions) == 0 or positions.shape[1] == 3:
            comps[0]["rerun.vec3d"] = Vec3DArray.from_numpy(positions)
        else:
            raise TypeError("position should be 1x3")

    if rotations_q is not None:
        rotations_q = np.require(rotations_q, dtype="float32")

        if len(rotations_q) == 0 or rotations_q.shape[1] == 4:
            comps[0]["rerun.quaternion"] = QuaternionArray.from_numpy(rotations_q)
        else:
            raise TypeError("rotation should be 1x4")

    if len(colors):
        is_splat = len(colors.shape) == 1
        if is_splat:
            colors = colors.reshape(1, len(colors))
        comps[is_splat]["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if len(radii):
        is_splat = len(radii) == 1
        comps[is_splat]["rerun.radius"] = RadiusArray.from_numpy(radii)

    if len(labels):
        is_splat = len(labels) == 1
        comps[is_splat]["rerun.label"] = LabelArray.new(labels)

    if len(class_ids):
        is_splat = len(class_ids) == 1
        comps[is_splat]["rerun.class_id"] = ClassIdArray.from_numpy(class_ids)

    if ext:
        _add_extension_components(comps[0], comps[1], ext, None)

    if comps[1]:
        comps[1]["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=comps[1], timeless=timeless, recording=recording)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    bindings.log_arrow_msg(entity_path, components=comps[0], timeless=timeless, recording=recording)
