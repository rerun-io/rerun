from typing import Optional, Tuple

import numpy.typing as npt
from rerun.log import _to_sequence

from rerun import rerun_bindings  # type: ignore[attr-defined]

__all__ = [
    "log_view_coordinates",
    "log_unknown_transform",
    "log_rigid3",
]


def log_view_coordinates(
    obj_path: str,
    *,
    xyz: str = "",
    up: str = "",
    right_handed: Optional[bool] = None,
    timeless: bool = False,
) -> None:
    """
    Log the view coordinates for an object.

    Each object defines its own coordinate system, called a space.
    By logging view coordinates you can give semantic meaning to the XYZ axes of the space.
    This is for instance useful for camera objects ("what axis is forward?").

    For full control, set the `xyz` parameter to a three-letter acronym (`xyz="RDF"`). Each letter represents:

    * R: Right
    * L: Left
    * U: Up
    * D: Down
    * F: Forward
    * B: Back

    Some of the most common are:

    * "RDF": X=Right Y=Down Z=Forward  (right-handed)
    * "RUB"  X=Right Y=Up   Z=Back     (right-handed)
    * "RDB": X=Right Y=Down Z=Back     (left-handed)
    * "RUF": X=Right Y=Up   Z=Forward  (left-handed)

    Example
    -------
    ```
    rerun.log_view_coordinates("world/camera", xyz="RUB")
    ```

    For world-coordinates it's often conventient to just specify an up-axis.
    You can do so by using the `up`-parameter (where `up` is one of "+X", "-X", "+Y", "-Y", "+Z", "-Z"):

    ```
    rerun.log_view_coordinates("world", up="+Z", right_handed=True, timeless=True)
    rerun.log_view_coordinates("world", up="-Y", right_handed=False, timeless=True)
    ```

    """
    if xyz == "" and up == "":
        raise TypeError("You must set either 'xyz' or 'up'")
    if xyz != "" and up != "":
        raise TypeError("You must set either 'xyz' or 'up', but not both")
    if xyz != "":
        rerun_bindings.log_view_coordinates_xyz(obj_path, xyz, right_handed, timeless)
    else:
        if right_handed is None:
            right_handed = True
        rerun_bindings.log_view_coordinates_up_handedness(obj_path, up, right_handed, timeless)


def log_unknown_transform(obj_path: str, timeless: bool = False) -> None:
    """Log that this object is NOT in the same space as the parent, but you do not (yet) know how they relate."""
    rerun_bindings.log_unknown_transform(obj_path, timeless=timeless)


def log_rigid3(
    obj_path: str,
    *,
    parent_from_child: Optional[Tuple[npt.ArrayLike, npt.ArrayLike]] = None,
    child_from_parent: Optional[Tuple[npt.ArrayLike, npt.ArrayLike]] = None,
    xyz: str = "",
    timeless: bool = False,
) -> None:
    """
    Log a proper rigid 3D transform between this object and the parent.

    Set either `parent_from_child` or `child_from_parent` to
    a tuple of `(translation_xyz, quat_xyzw)`.

    `parent_from_child`
    -------------------
    `parent_from_child=(translation_xyz, quat_xyzw)`

    Also known as pose (e.g. camera extrinsics).

    The translation is the position of the object in the parent space.
    The resulting transform from child to parent corresponds to taking a point in the child space,
    rotating it by the given rotations, and then translating it by the given translation:

    `point_parent = translation + quat * point_child * quat*

    `child_from_parent`
    -------------------
    `child_from_parent=(translation_xyz, quat_xyzw)`

    the inverse of `parent_from_child`

    `xyz`
    ----
    Optionally set the view coordinates of this object, e.g. to `RDF` for `X=Right, Y=Down, Z=Forward`.
    This is a convenience for also calling `log_view_coordinates`.

    Example
    -------
    ```
    rerun.log_rigid3("world/camera", parent_from_child=(t,q))
    rerun.log_pinhole("world/camera/image", …)
    ```

    """
    if parent_from_child and child_from_parent:
        raise TypeError("Set either parent_from_child or child_from_parent, but not both")
    elif parent_from_child:
        (t, q) = parent_from_child
        rerun_bindings.log_rigid3(
            obj_path,
            parent_from_child=True,
            rotation_q=_to_sequence(q),
            translation=_to_sequence(t),
            timeless=timeless,
        )
    elif child_from_parent:
        (t, q) = child_from_parent
        rerun_bindings.log_rigid3(
            obj_path,
            parent_from_child=False,
            rotation_q=_to_sequence(q),
            translation=_to_sequence(t),
            timeless=timeless,
        )
    else:
        raise TypeError("Set either parent_from_child or child_from_parent")

    if xyz != "":
        log_view_coordinates(obj_path, xyz=xyz, timeless=timeless)
