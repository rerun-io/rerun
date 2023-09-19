from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import numpy.typing as npt

from rerun.log import (
    Color,
    Colors,
    OptionalClassIds,
)
from rerun.log_deprecated.log_decorator import log_decorator
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
    log_obbs(
        entity_path,
        half_sizes=half_size,
        positions=position,
        rotations_q=rotation_q,
        colors=color,
        stroke_widths=stroke_width,
        labels=label,
        class_ids=class_id,
        ext=ext,
        timeless=timeless,
        recording=recording,
    )


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
    class_ids: OptionalClassIds = None,
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
    from rerun.experimental import Boxes3D, dt, log

    if half_sizes is None:
        raise ValueError("`half_sizes` argument must be set")

    if np.any(half_sizes):  # type: ignore[arg-type]
        half_sizes = np.asarray(half_sizes, dtype="float32")
        if half_sizes.ndim == 1:
            half_sizes = np.expand_dims(half_sizes, axis=0)
    else:
        half_sizes = np.zeros((0, 4), dtype="float32")
    assert type(half_sizes) is np.ndarray

    recording = RecordingStream.to_native(recording)

    if rotations_q is not None:
        rotations_q = np.asarray(rotations_q, dtype="float32")
        if rotations_q.ndim == 1:
            rotations_q = np.expand_dims(rotations_q, axis=0)
        rotations = [dt.Quaternion(xyzw=quat) for quat in rotations_q]
    else:
        rotations = None

    if stroke_widths is not None:
        radii = np.asarray(stroke_widths, dtype="float32")
        radii /= 2.0
    else:
        radii = None

    arch = Boxes3D(
        half_sizes=half_sizes,
        centers=positions,
        rotations=rotations,
        radii=radii,
        colors=colors,
        labels=labels,
        class_ids=class_ids,
    )
    return log(entity_path, arch, ext=ext, timeless=timeless, recording=recording)
