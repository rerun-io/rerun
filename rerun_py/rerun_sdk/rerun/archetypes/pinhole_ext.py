from __future__ import annotations

import math
from typing import Any, cast

import numpy.typing as npt

from ..datatypes import Mat3x3Like, Vec2D, Vec2DLike, ViewCoordinatesLike
from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions


class PinholeExt:
    """Extension for [Pinhole][rerun.archetypes.Pinhole]."""

    def __init__(
        self: Any,
        *,
        image_from_camera: Mat3x3Like | None = None,
        resolution: Vec2DLike | None = None,
        camera_xyz: ViewCoordinatesLike | None = None,
        width: int | float | None = None,
        height: int | float | None = None,
        focal_length: float | npt.ArrayLike | None = None,
        principal_point: npt.ArrayLike | None = None,
        fov_y: float | None = None,
        aspect_ratio: float | None = None,
        image_plane_distance: float | None = None,
    ) -> None:
        """
        Create a new instance of the Pinhole archetype.

        Parameters
        ----------
        image_from_camera:
            Row-major intrinsics matrix for projecting from camera space to image space.
            The first two axes are X=Right and Y=Down, respectively.
            Projection is done along the positive third (Z=Forward) axis.
            This can be specified _instead_ of `focal_length` and `principal_point`.
        resolution:
            Pixel resolution (usually integers) of child image space. Width and height.
            `image_from_camera` projects onto the space spanned by `(0,0)` and `resolution - 1`.
        camera_xyz:
            Sets the view coordinates for the camera.

            All common values are available as constants on the `components.ViewCoordinates` class.

            The default is `ViewCoordinates.RDF`, i.e. X=Right, Y=Down, Z=Forward, and this is also the recommended setting.
            This means that the camera frustum will point along the positive Z axis of the parent space,
            and the cameras "up" direction will be along the negative Y axis of the parent space.

            The camera frustum will point whichever axis is set to `F` (or the opposite of `B`).
            When logging a depth image under this entity, this is the direction the point cloud will be projected.
            With `RDF`, the default forward is +Z.

            The frustum's "up" direction will be whichever axis is set to `U` (or the opposite of `D`).
            This will match the negative Y direction of pixel space (all images are assumed to have xyz=RDF).
            With `RDF`, the default is up is -Y.

            The frustum's "right" direction will be whichever axis is set to `R` (or the opposite of `L`).
            This will match the positive X direction of pixel space (all images are assumed to have xyz=RDF).
            With `RDF`, the default right is +x.

            Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left, Z=Up).

            NOTE: setting this to something else than `RDF` (the default) will change the orientation of the camera frustum,
            and make the pinhole matrix not match up with the coordinate system of the pinhole entity.

            The pinhole matrix (the `image_from_camera` argument) always project along the third (Z) axis,
            but will be re-oriented to project along the forward axis of the `camera_xyz` argument.
        focal_length:
            The focal length of the camera in pixels.
            This is the diagonal of the projection matrix.
            Set one value for symmetric cameras, or two values (X=Right, Y=Down) for anamorphic cameras.
        principal_point:
            The center of the camera in pixels.
            The default is half the width and height.
            This is the last column of the projection matrix.
            Expects two values along the dimensions Right and Down
        width:
            Width of the image in pixels.
        height:
            Height of the image in pixels.
        fov_y:
            Vertical field of view in radians.
        aspect_ratio
            Aspect ratio (width/height).
        image_plane_distance:
            The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.
            This is only used for visualization purposes, and does not affect the projection itself.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if resolution is None and width is not None and height is not None:
                resolution = [width, height]
            elif resolution is not None and (width is not None or height is not None):
                _send_warning_or_raise("Can't set both resolution and width/height", 1)

            # TODO(andreas): Use a union type for the Pinhole component instead ~Zof converting to a matrix here
            if image_from_camera is None:
                if fov_y is not None and aspect_ratio is not None:
                    EPSILON = 1.19209e-07
                    focal_length = focal_length = 0.5 / math.tan(max(fov_y * 0.5, EPSILON))
                    resolution = [aspect_ratio, 1.0]

                if resolution is not None:
                    res_vec = Vec2D(resolution)
                    width = cast(float, res_vec.xy[0])
                    height = cast(float, res_vec.xy[1])
                else:
                    width = None
                    height = None

                if focal_length is None:
                    if height is None or width is None:
                        raise ValueError("Either image_from_camera or focal_length must be set")
                    else:
                        _send_warning_or_raise("Either image_from_camera or focal_length must be set", 1)
                        focal_length = (width * height) ** 0.5  # a reasonable best-effort default

                if principal_point is None:
                    if height is not None and width is not None:
                        principal_point = [width / 2, height / 2]
                    else:
                        raise ValueError("Must provide one of principal_point, resolution, or width/height")

                if type(focal_length) in (int, float):
                    fl_x = focal_length
                    fl_y = focal_length
                else:
                    try:
                        # TODO(emilk): check that it is 2 elements long
                        fl_x = focal_length[0]  # type: ignore[index]
                        fl_y = focal_length[1]  # type: ignore[index]
                    except Exception:
                        raise ValueError("Expected focal_length to be one or two floats")

                try:
                    u_cen = principal_point[0]  # type: ignore[index]
                    v_cen = principal_point[1]  # type: ignore[index]
                except Exception:
                    raise ValueError("Expected principal_point to be one or two floats")

                image_from_camera = [[fl_x, 0, u_cen], [0, fl_y, v_cen], [0, 0, 1]]  # type: ignore[assignment]
            else:
                if focal_length is not None:
                    _send_warning_or_raise("Both image_from_camera and focal_length set", 1)
                if principal_point is not None:
                    _send_warning_or_raise("Both image_from_camera and principal_point set", 1)
                if fov_y is not None or aspect_ratio is not None:
                    _send_warning_or_raise("Both image_from_camera and fov_y or aspect_ratio set", 1)

            self.__attrs_init__(
                image_from_camera=image_from_camera,
                resolution=resolution,
                camera_xyz=camera_xyz,
                image_plane_distance=image_plane_distance,
            )
            return

        self.__attrs_clear__()
