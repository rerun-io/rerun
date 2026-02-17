from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    import numpy.typing as npt

    from ..datatypes import Float32Like, Mat3x3Like, Rgba32Like, Utf8Like, Vec2DLike, Vec4DLike, ViewCoordinatesLike


class FisheyeExt:
    """Extension for [Fisheye][rerun.archetypes.Fisheye]."""

    def __init__(
        self: Any,
        *,
        image_from_camera: Mat3x3Like | None = None,
        resolution: Vec2DLike | None = None,
        distortion_coefficients: Vec4DLike | None = None,
        camera_xyz: ViewCoordinatesLike | None = None,
        width: int | float | None = None,
        height: int | float | None = None,
        focal_length: float | npt.ArrayLike | None = None,
        principal_point: npt.ArrayLike | None = None,
        child_frame: Utf8Like | None = None,
        parent_frame: Utf8Like | None = None,
        image_plane_distance: float | None = None,
        color: Rgba32Like | None = None,
        line_width: Float32Like | None = None,
    ) -> None:
        """
        Create a new instance of the Fisheye archetype.

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
        distortion_coefficients:
            Fisheye radial distortion coefficients `[k1, k2, k3, k4]` for the equidistant model.
            Defaults to `[0, 0, 0, 0]`.
        camera_xyz:
            Sets the view coordinates for the camera.

            All common values are available as constants on the `components.ViewCoordinates` class.

            The default is `ViewCoordinates.RDF`, i.e. X=Right, Y=Down, Z=Forward,
            and this is also the recommended setting.
        focal_length:
            The focal length of the camera in pixels.
            This is the diagonal of the projection matrix.
            Set one value for symmetric cameras, or two values (X=Right, Y=Down) for anamorphic cameras.
        principal_point:
            The center of the camera in pixels.
            The default is half the width and height.
            This is the last column of the projection matrix.
            Expects two values along the dimensions Right and Down.
        width:
            Width of the image in pixels.
        height:
            Height of the image in pixels.
        child_frame:
            The child frame this transform transforms from.
        parent_frame:
            The parent frame this transform transforms into.
        image_plane_distance:
            The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.

            This is only used for visualization purposes, and does not affect the projection itself.
        color:
            Color of the camera frustum lines in the 3D viewer.
        line_width:
            Width of the camera frustum lines in the 3D viewer.

        """

        from ..datatypes import Vec2D

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if resolution is None and width is not None and height is not None:
                resolution = [width, height]
            elif resolution is not None and (width is not None or height is not None):
                _send_warning_or_raise("Can't set both resolution and width/height", 1)

            if image_from_camera is None:
                if resolution is not None:
                    res_vec = Vec2D(resolution)
                    width = cast("float", res_vec.xy[0])
                    height = cast("float", res_vec.xy[1])
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
                        fl_x = focal_length[0]  # type: ignore[index]
                        fl_y = focal_length[1]  # type: ignore[index]
                    except Exception:
                        raise ValueError("Expected focal_length to be one or two floats") from None

                try:
                    u_cen = principal_point[0]  # type: ignore[index]
                    v_cen = principal_point[1]  # type: ignore[index]
                except Exception:
                    raise ValueError("Expected principal_point to be one or two floats") from None

                image_from_camera = [[fl_x, 0, u_cen], [0, fl_y, v_cen], [0, 0, 1]]  # type: ignore[assignment]
            else:
                if focal_length is not None:
                    _send_warning_or_raise("Both image_from_camera and focal_length set", 1)
                if principal_point is not None:
                    _send_warning_or_raise("Both image_from_camera and principal_point set", 1)

            self.__attrs_init__(
                image_from_camera=image_from_camera,
                resolution=resolution,
                distortion_coefficients=distortion_coefficients,
                camera_xyz=camera_xyz,
                child_frame=child_frame,
                parent_frame=parent_frame,
                image_plane_distance=image_plane_distance,
                color=color,
                line_width=line_width,
            )
            return

        self.__attrs_clear__()
