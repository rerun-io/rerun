from __future__ import annotations

from typing import Any, cast

import numpy.typing as npt

from rerun.log.error_utils import _send_warning

from ..datatypes.mat3x3 import Mat3x3Like
from ..datatypes.vec2d import Vec2D, Vec2DLike


class PinholeExt:
    """
    Log a perspective camera model.

    Parameters
    ----------
    image_from_cam:
        Column-major projection matrix.

        Child from parent.
        Image coordinates from camera view coordinates.

        Example:
        ```text
        [[1496.1, 0.0,    0.0], // col 0
        [0.0,    1496.1, 0.0], // col 1
        [980.5,  744.5,  1.0]] // col 2
        ```
    resolution:
        Pixel resolution (usually integers) of child image space. Width and height.
        `image_from_cam` projects onto the space spanned by `(0,0)` and `resolution - 1`.
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
    """

    def __init__(
        self: Any,
        image_from_cam: Mat3x3Like | None = None,
        resolution: Vec2DLike | None = None,
        width: int | float | None = None,
        height: int | float | None = None,
        focal_length_px: float | npt.ArrayLike | None = None,
        principal_point_px: npt.ArrayLike | None = None,
    ) -> None:
        if resolution is None:
            if width is None or height is None:
                _send_warning("log_pinhole: either resolution or width and height must be set", 1)
            resolution = [width or 1, height or 1]

        # TODO(andreas): Use a union type for the Pinhole component instead ~Zof converting to a matrix here
        if image_from_cam is None:
            resolution = Vec2D(resolution)
            width = cast(float, resolution.xy[0])
            height = cast(float, resolution.xy[1])

            if focal_length_px is None:
                _send_warning("log_pinhole: either child_from_parent or focal_length_px must be set", 1)
                focal_length_px = (width * height) ** 0.5  # a reasonable default
            if principal_point_px is None:
                principal_point_px = [width / 2, height / 2]
            if type(focal_length_px) in (int, float):
                fl_x = focal_length_px
                fl_y = focal_length_px
            else:
                try:
                    # TODO(emilk): check that it is 2 elements long
                    fl_x = focal_length_px[0]  # type: ignore[index]
                    fl_y = focal_length_px[1]  # type: ignore[index]
                except Exception:
                    _send_warning("log_pinhole: expected focal_length_px to be one or two floats", 1)
                    fl_x = width / 2
                    fl_y = fl_x

            try:
                u_cen = principal_point_px[0]  # type: ignore[index]
                v_cen = principal_point_px[1]  # type: ignore[index]
            except Exception:
                _send_warning("log_pinhole: expected principal_point_px to be one or two floats", 1)
                u_cen = width / 2
                v_cen = height / 2

            image_from_cam = [[fl_x, 0, u_cen], [0, fl_y, v_cen], [0, 0, 1]]  # type: ignore[assignment]
        else:
            if focal_length_px is not None:
                _send_warning("log_pinhole: both child_from_parent and focal_length_px set", 1)
            if principal_point_px is not None:
                _send_warning("log_pinhole: both child_from_parent and principal_point_px set", 1)

        self.__attrs_init__(image_from_cam=image_from_cam, resolution=resolution)
