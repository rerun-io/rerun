import numpy as np
import numpy.typing as npt

from rerun import bindings

__all__ = [
    "log_pinhole",
]


def log_pinhole(
    entity_path: str, *, child_from_parent: npt.ArrayLike, width: int, height: int, timeless: bool = False
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
    rerun.log_rigid3("world/camera", …)
    rerun.log_pinhole("world/camera/image", …)
    ```

    Parameters
    ----------
    entity_path:
        Path to the child (image) space in the space hierarchy.
    child_from_parent:
        Row-major intrinsics matrix for projecting from camera space to image space.
    width:
        Horizontal resolution of the child space.
    height:
        Vertical resolution of the child space.
    timeless:
        If true, the camera will be timeless (default: False).

    """

    if not bindings.is_enabled():
        return

    # Transform arrow handling happens inside the python bridge
    bindings.log_pinhole(
        entity_path,
        resolution=[width, height],
        child_from_parent=np.asarray(child_from_parent).T.tolist(),
        timeless=timeless,
    )
