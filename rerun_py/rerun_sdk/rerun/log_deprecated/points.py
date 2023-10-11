from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import numpy.typing as npt
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.any_value import AnyValues
from rerun.archetypes import Points2D, Points3D
from rerun.error_utils import _send_warning_or_raise
from rerun.log_deprecated import (
    Color,
    Colors,
    OptionalClassIds,
    OptionalKeyPointIds,
)
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_point",
    "log_points",
]


@deprecated(
    """Please migrate to `rr.log(…, rr.Points2D(…))` or `rr.log(…, rr.Points3D(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_point(
    entity_path: str,
    position: npt.ArrayLike,
    *,
    radius: float | None = None,
    color: Color | None = None,
    label: str | None = None,
    class_id: int | None = None,
    keypoint_id: int | None = None,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a 2D or 3D point, with a position and optional color, radii, label, etc.

    !!! Warning "Deprecated"
        Please migrate to [rerun.Points2D][] or [rerun.Points3D][]

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

    Logging again to the same `entity_path` will replace the previous point.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA represented as a 2-element or 3-element sequence.

    Supported dtypes for `color`:
    -----------------------------
     - uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
     - float32/float64: all color components should be in 0-1 linear space.

    Parameters
    ----------
    entity_path:
        Path to the point in the space hierarchy.
    position:
        Any 2-element or 3-element array-like.
    radius:
        Optional radius (make it a sphere).
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    label:
        Optional text to show with the point.
    class_id:
        Optional class id for the point.
        The class id provides color and label if not specified explicitly.
        See [rerun.log_annotation_context][]
    keypoint_id:
        Optional key point id for the point, identifying it within a class.
        If keypoint_id is passed but no class_id was specified, class_id will be set to 0.
        This is useful to identify points within a single classification (which is identified with class_id).
        E.g. the classification might be 'Person' and the keypoints refer to joints on a detected skeleton.
        See [rerun.log_annotation_context][]
    draw_order:
        An optional floating point value that specifies the 2D drawing order for 2D points.
        Objects with higher values are drawn on top of those with lower values.
        The default for 2D points is 30.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the point will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    """

    if position is None:
        raise ValueError("`position` argument must be set")

    recording = RecordingStream.to_native(recording)

    if keypoint_id is not None and class_id is None:
        class_id = 0

    position = np.require(position, dtype="float32")
    if position.size == 2:
        points2d = Points2D(
            positions=position,
            radii=radius,
            colors=color,
            labels=label,
            draw_order=draw_order,
            class_ids=class_id,
            keypoint_ids=keypoint_id,
        )
        return log(entity_path, points2d, AnyValues(**(ext or {})), timeless=timeless, recording=recording)
    elif position.size == 3:
        if draw_order is not None:
            raise ValueError("`draw_order` is only supported for 3D points")
        points3d = Points3D(
            positions=position,
            radii=radius,
            colors=color,
            labels=label,
            class_ids=class_id,
            keypoint_ids=keypoint_id,
        )
        return log(entity_path, points3d, AnyValues(**(ext or {})), timeless=timeless, recording=recording)
    else:
        raise TypeError("Position must have a total size of 2 or 3")


@deprecated(
    """Please migrate to `rr.log(…, rr.Points2D(…))` or `rr.log(…, rr.Points3D(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_points(
    entity_path: str,
    positions: npt.ArrayLike,
    *,
    identifiers: npt.ArrayLike | None = None,
    colors: Color | Colors | None = None,
    radii: npt.ArrayLike | None = None,
    labels: Sequence[str] | None = None,
    class_ids: OptionalClassIds = None,
    keypoint_ids: OptionalKeyPointIds = None,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log 2D or 3D points, with positions and optional colors, radii, labels, etc.

    !!! Warning "Deprecated"
        Please migrate to [rerun.Points2D][] or [rerun.Points3D][]

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

    Logging again to the same `entity_path` will replace all the previous points.

    Colors should either be in 0-255 gamma space or in 0-1 linear space.
    Colors can be RGB or RGBA. You can supply no colors, one color,
    or one color per point in a Nx3 or Nx4 numpy array.

    Supported dtypes for `colors`:
    ------------------------------
     - uint8: color components should be in 0-255 sRGB gamma space, except for alpha which should be in 0-255 linear
    space.
     - float32/float64: all color components should be in 0-1 linear space.


    Parameters
    ----------
    entity_path:
        Path to the points in the space hierarchy.
    positions:
        Nx2 or Nx3 array
    identifiers:
        Unique numeric id that shows up when you hover or select the point.
    colors:
        Optional colors of the points.
        The colors are interpreted as RGB or RGBA in sRGB gamma-space,
        as either 0-1 floats or 0-255 integers, with separate alpha.
    radii:
        Optional radii (make it a sphere).
    labels:
        Optional per-point text to show with the points
    class_ids:
        Optional class ids for the points.
        The class id provides colors and labels if not specified explicitly.
        See [rerun.log_annotation_context][]
    keypoint_ids:
        Optional key point ids for the points, identifying them within a class.
        If keypoint_ids are passed in but no class_ids were specified, class_id will be set to 0.
        This is useful to identify points within a single classification (which is identified with class_id).
        E.g. the classification might be 'Person' and the keypoints refer to joints on a detected skeleton.
        See [rerun.log_annotation_context][]
    draw_order:
        An optional floating point value that specifies the 2D drawing order for 2D points.
        Objects with higher values are drawn on top of those with lower values.
        The default for 2D points is 30.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the points will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    """

    if positions is None:
        raise ValueError("`positions` argument must be set")

    recording = RecordingStream.to_native(recording)

    if keypoint_ids is not None and class_ids is None:
        class_ids = 0

    positions = np.require(positions, dtype="float32")

    if positions.shape[0] == 0:
        # We used to support sending zero points and a long list of radii, but no more
        radii = None
        colors = None
        labels = None
        class_ids = None
        keypoint_ids = None
        identifiers = None

    identifiers_np = None
    if identifiers is not None:
        try:
            identifiers_np = np.require(identifiers, dtype="uint64")
        except ValueError:
            _send_warning_or_raise("Only integer identifiers supported", 1)

    if positions.shape[1] == 2:
        points2d = Points2D(
            positions=positions,
            radii=radii,
            colors=colors,
            labels=labels,
            draw_order=draw_order,
            class_ids=class_ids,
            keypoint_ids=keypoint_ids,
            instance_keys=identifiers_np,
        )
        return log(entity_path, points2d, AnyValues(**(ext or {})), timeless=timeless, recording=recording)
    elif positions.shape[1] == 3:
        if draw_order is not None:
            raise ValueError("`draw_order` is only supported for 3D points")

        points3d = Points3D(
            positions=positions,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            keypoint_ids=keypoint_ids,
            instance_keys=identifiers_np,
        )
        return log(entity_path, points3d, AnyValues(**(ext or {})), timeless=timeless, recording=recording)
    else:
        raise TypeError("Positions should be Nx2 or Nx3")
