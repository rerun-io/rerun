"""
Methods for logging transforms on entity paths.

Learn more about transforms [in the manual](https://www.rerun.io/docs/concepts/spaces-and-transforms)
"""
from typing import Optional, Tuple

import numpy.typing as npt

from rerun import bindings
from rerun.log import _to_sequence
from rerun.log.error_utils import _send_warning
from rerun.log.log_decorator import log_decorator

__all__ = [
    "log_view_coordinates",
    "log_unknown_transform",
    "log_rigid3",
]


@log_decorator
def log_view_coordinates(
    entity_path: str,
    *,
    xyz: Optional[Tuple[bindings.ViewDir, bindings.ViewDir, bindings.ViewDir]] = None,
    up: Optional[Tuple[bindings.Sign, bindings.Axis3]] = None,
    right_handed: Optional[bool] = None,
    timeless: bool = False,
) -> None:
    """
    Log the view coordinates for an entity.

    Each entity defines its own coordinate system, called a space.
    By logging view coordinates you can give semantic meaning to the XYZ axes of the space.
    This is for example useful for camera entities ("what axis is forward?").

    For full control, set the `xyz` parameter to a three-component tuple (X, Y, Z).
    Each component can be of the following variant: Right | Left | Up | Down | Forward | Back.

    Some of the most common are:

    * (Right, Down, Forward): X=Right Y=Down Z=Forward  (right-handed)
    * (Right, Up, Back)  X=Right Y=Up   Z=Back     (right-handed)
    * (Right, Down, Back): X=Right Y=Down Z=Back     (left-handed)
    * (Right, Up, Forward): X=Right Y=Up   Z=Forward  (left-handed)

    Example
    -------
    ```
    rerun.log_view_coordinates("world/camera", xyz=(rerun.bindings.ViewDir.Right, rerun.bindings.ViewDir.Up, rerun.bindings.ViewDir.Back))
    ```

    For world-coordinates it's often convenient to just specify an up-axis.
    You can do so by using the `up`-parameter:

    ```
    rerun.log_view_coordinates("world", up=(rerun.bindings.Sign.Positive, rerun.bindings.Axis3.Z), right_handed=True, timeless=True)
    rerun.log_view_coordinates("world", up=(rerun.bindings.Sign.Negative, rerun.bindings.Axis3.Y), right_handed=False, timeless=True)
    ```

    Parameters
    ----------
    entity_path:
        Path in the space hierarchy where the view coordinate will be set.
    xyz:
        Three-component tuple for the view coordinate axes (X,Y,Z).
    up:
        Which axis is up? Defined by a tuple with (Axis: X|Y|Z, Sign:Positive|Negative).
    right_handed:
        If True, the coordinate system is right-handed. If False, it is left-handed.
    timeless:
        If true, the view coordinates will be timeless (default: False).

    """
    if xyz is None and up is None:
        _send_warning("You must set either 'xyz' or 'up'. Ignoring log.", 1)
        return
    if xyz is not None and up is not None:
        _send_warning("You must set either 'xyz' or 'up', but not both. Dropping up.", 1)
        up = None
    if xyz is not None:
        bindings.log_view_coordinates_xyz(entity_path, xyz, right_handed, timeless)
    else:
        if right_handed is None:
            right_handed = True
        bindings.log_view_coordinates_up_handedness(entity_path, up, right_handed, timeless)


@log_decorator
def log_unknown_transform(entity_path: str, timeless: bool = False) -> None:
    """Log that this entity is NOT in the same space as the parent, but you do not (yet) know how they relate."""

    bindings.log_unknown_transform(entity_path, timeless=timeless)


@log_decorator
def log_rigid3(
    entity_path: str,
    *,
    parent_from_child: Optional[Tuple[npt.ArrayLike, npt.ArrayLike]] = None,
    child_from_parent: Optional[Tuple[npt.ArrayLike, npt.ArrayLike]] = None,
    xyz: Optional[Tuple[bindings.ViewDir, bindings.ViewDir, bindings.ViewDir]] = None,
    timeless: bool = False,
) -> None:
    """
    Log a proper rigid 3D transform between this entity and the parent.

    Set either `parent_from_child` or `child_from_parent` to a tuple of `(translation_xyz, quat_xyzw)`.

    Parent-from-child
    -----------------
    Also known as pose (e.g. camera extrinsics).

    The translation is the position of the entity in the parent space.
    The resulting transform from child to parent corresponds to taking a point in the child space,
    rotating it by the given rotations, and then translating it by the given translation:

    `point_parent = translation + quat * point_child * quat*`

    Example
    -------
    ```
    t = 0.0
    translation = [math.sin(t), math.cos(t), 0.0] # circle around origin
    rotation = [0.5, 0.0, 0.0, np.sin(np.pi/3)] # 60 degrees around x-axis
    rerun.log_rigid3("sun/planet", parent_from_child=(translation, rotation))
    ```

    Parameters
    ----------
    entity_path:
        Path of the *child* space in the space hierarchy.
    parent_from_child:
        A tuple of `(translation_xyz, quat_xyzw)` mapping points in the child space to the parent space.
    child_from_parent:
        the inverse of `parent_from_child`
    xyz:
        Optionally set the view coordinates of this entity, e.g. to `RDF` for `X=Right, Y=Down, Z=Forward`.
        This is a convenience for also calling [log_view_coordinates][rerun.log_view_coordinates].
    timeless:
        If true, the transform will be timeless (default: False).

    """

    if parent_from_child and child_from_parent:
        raise TypeError("Set either parent_from_child or child_from_parent, but not both.")

    if parent_from_child:
        (t, q) = parent_from_child
        bindings.log_rigid3(
            entity_path,
            parent_from_child=True,
            rotation_q=_to_sequence(q),
            translation=_to_sequence(t),
            timeless=timeless,
        )
    elif child_from_parent:
        (t, q) = child_from_parent
        bindings.log_rigid3(
            entity_path,
            parent_from_child=False,
            rotation_q=_to_sequence(q),
            translation=_to_sequence(t),
            timeless=timeless,
        )
    else:
        raise TypeError("Set either parent_from_child or child_from_parent.")

    if xyz is not None:
        log_view_coordinates(entity_path, xyz=xyz, timeless=timeless)
