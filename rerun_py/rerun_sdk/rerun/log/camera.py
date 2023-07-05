from __future__ import annotations

import numpy.typing as npt

from rerun import bindings
from rerun.components.pinhole import Pinhole, PinholeArray
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_pinhole",
]


@log_decorator
def log_pinhole(
    entity_path: str,
    *,
    child_from_parent: npt.ArrayLike,
    width: int,
    height: int,
    timeless: bool = False,
    recording: RecordingStream | None = None,
    camera_xyz: str | None = None,
) -> None:
    """
    Log a perspective camera model.

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
    u_cen = width / 2
    v_cen = height / 2
    f_len = (height * width) ** 0.5

    rerun.log_pinhole("world/camera/image",
                      child_from_parent = [[f_len, 0,     u_cen],
                                           [0,     f_len, v_cen],
                                           [0,     0,     1  ]],
                      width = width,
                      height = height,
                      camera_xyz="RDF")
    ```

    Parameters
    ----------
    entity_path:
        Path to the child (image) space in the space hierarchy.
    child_from_parent:
        Row-major intrinsics matrix for projecting from camera space to image space.
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
        Sets the view coordinates for the camera. The default is "RDF", i.e. X=Right, Y=Down, Z=Forward.
        Other common formats are "RUB" (X=Right, Y=Up, Z=Back) and "FLU" (X=Forward, Y=Left, Z=Up).
        Equivalent to calling [`rerun.log_view_coordinates(entity, xyz=…)`][rerun.log_view_coordinates].
        This will change the orientation of the camera frustum.
        NOTE: setting this to something else than "RDF" (the default) will change the orientation of the camera frustum,
        and make the pinhole matrix not match up with the coordinate system of the pinhole entity.
        The pinhole matrix (the `child_from_parent` argument) always project along the Z axis of the camera space,
        but will be re-oritented to project along another axis if the `camera_xyz` argument is set.

    """

    instanced = {"rerun.pinhole": PinholeArray.from_pinhole(Pinhole(child_from_parent, [width, height]))}
    bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless)

    if camera_xyz:
        bindings.log_view_coordinates_xyz(
            entity_path,
            xyz=camera_xyz,
            timeless=timeless,
            recording=recording,
        )
