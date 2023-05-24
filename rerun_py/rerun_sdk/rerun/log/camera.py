from typing import Optional

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
    recording: Optional[RecordingStream] = None,
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
                      height = height)
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

    """

    instanced = {"rerun.pinhole": PinholeArray.from_pinhole(Pinhole(child_from_parent, [width, height]))}
    bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless)
