from __future__ import annotations

import numpy.typing as npt
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.archetypes import Pinhole
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

from .transform import log_view_coordinates

__all__ = [
    "log_pinhole",
]


@deprecated(
    """Please migrate to `rr.log(…, rr.Pinhole(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_pinhole(
    entity_path: str,
    *,
    width: int,
    height: int,
    focal_length_px: float | npt.ArrayLike | None = None,
    principal_point_px: npt.ArrayLike | None = None,
    child_from_parent: npt.ArrayLike | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
    camera_xyz: str | None = None,
) -> None:
    """
    Log a perspective camera model.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Pinhole][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

    This logs the pinhole model that projects points from the parent (camera) space to this space (image) such that:
    ```
    point_image_hom = child_from_parent * point_cam
    point_image = point_image_hom[:,1] / point_image_hom[2]
    ```

    Where `point_image_hom` is the projected point in the image space expressed in homogeneous coordinates.

    Example
    -------
    ```
    width = 640
    height = 480
    f_len = (height * width) ** 0.5

    rerun.log_pinhole("world/camera/image",
                      width = width,
                      height = height,
                      focal_length_px = f_len)

    # More explicit:
    u_cen = width / 2
    v_cen = height / 2
    rerun.log_pinhole("world/camera/image",
                      width = width,
                      height = height,
                      child_from_parent = [[f_len, 0,     u_cen],
                                           [0,     f_len, v_cen],
                                           [0,     0,     1  ]],
                      camera_xyz="RDF")
    ```

    Parameters
    ----------
    entity_path:
        Path to the child (image) space in the space hierarchy.
    focal_length_px:
        The focal length of the camera in pixels.
        This is the diagonal of the projection matrix.
        Set one value for symmetric cameras, or two values (X=Right, Y=Down) for anamorphic cameras.
    principal_point_px:
        The center of the camera in pixels.
        The default is half the width and height.
        This is the last column of the projection matrix.
        Expects two values along the dimensions Right and Down
    child_from_parent:
        Row-major intrinsics matrix for projecting from camera space to image space.
        The first two axes are X=Right and Y=Down, respectively.
        Projection is done along the positive third (Z=Forward) axis.
        This can be specified _instead_ of `focal_length_px` and `principal_point_px`.
    width:
        Width of the image in pixels.
    height:
        Height of the image in pixels.
    timeless:
        If true, the camera will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    camera_xyz:
        Sets the view coordinates for the camera. The default is "RDF", i.e. X=Right, Y=Down, Z=Forward,
        and this is also the recommended setting.
        This means that the camera frustum will point along the positive Z axis of the parent space,
        and the cameras "up" direction will be along the negative Y axis of the parent space.

        Each letter represents:

        * R: Right
        * L: Left
        * U: Up
        * D: Down
        * F: Forward
        * B: Back

        The camera furstum will point whichever axis is set to `F` (or the oppositve of `B`).
        When logging a depth image under this entity, this is the direction the point cloud will be projected.
        With XYZ=RDF, the default forward is +Z.

        The frustum's "up" direction will be whichever axis is set to `U` (or the oppositve of `D`).
        This will match the negative Y direction of pixel space (all images are assumed to have xyz=RDF).
        With RDF, the default is up is -Y.

        The frustum's "right" direction will be whichever axis is set to `R` (or the oppositve of `L`).
        This will match the positive X direction of pixel space (all images are assumed to have xyz=RDF).
        With RDF, the default right is +x.

        Other common formats are "RUB" (X=Right, Y=Up, Z=Back) and "FLU" (X=Forward, Y=Left, Z=Up).

        Equivalent to calling [`rerun.log_view_coordinates(entity, xyz=…)`][rerun.log_view_coordinates].

        NOTE: setting this to something else than "RDF" (the default) will change the orientation of the camera frustum,
        and make the pinhole matrix not match up with the coordinate system of the pinhole entity.

        The pinhole matrix (the `child_from_parent` argument) always project along the third (Z) axis,
        but will be re-oriented to project along the forward axis of the `camera_xyz` argument.
    """

    arch = Pinhole(
        image_from_camera=child_from_parent,
        width=width,
        height=height,
        focal_length=focal_length_px,
        principal_point=principal_point_px,
    )
    log(entity_path, arch, timeless=timeless, recording=recording)

    if camera_xyz:
        log_view_coordinates(
            entity_path,
            xyz=camera_xyz,
            timeless=timeless,
            recording=recording,
        )
