"""
Methods for logging transforms on entity paths.

Learn more about transforms [in the manual](https://www.rerun.io/docs/concepts/spaces-and-transforms)
"""
from typing import Any, Dict, Optional, Tuple, Union

import numpy.typing as npt
from deprecated import deprecated

from rerun import bindings
from rerun.components.disconnected_space import DisconnectedSpaceArray
from rerun.components.transform3d import (
    RotationAxisAngle,
    Transform3D,
    Transform3DArray,
    TranslationAndMat3,
    TranslationRotationScale3D,
)
from rerun.log.error_utils import _send_warning
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_view_coordinates",
    "log_unknown_transform",
    "log_rigid3",
    "log_affine3",
]


@log_decorator
def log_view_coordinates(
    entity_path: str,
    *,
    xyz: str = "",
    up: str = "",
    right_handed: Optional[bool] = None,
    timeless: bool = False,
    recording: Optional[RecordingStream] = None,
) -> None:
    """
    Log the view coordinates for an entity.

    Each entity defines its own coordinate system, called a space.
    By logging view coordinates you can give semantic meaning to the XYZ axes of the space.
    This is for example useful for camera entities ("what axis is forward?").

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

    For world-coordinates it's often convenient to just specify an up-axis.
    You can do so by using the `up`-parameter (where `up` is one of "+X", "-X", "+Y", "-Y", "+Z", "-Z"):

    ```
    rerun.log_view_coordinates("world", up="+Z", right_handed=True, timeless=True)
    rerun.log_view_coordinates("world", up="-Y", right_handed=False, timeless=True)
    ```

    Parameters
    ----------
    entity_path:
        Path in the space hierarchy where the view coordinate will be set.
    xyz:
        Three-letter acronym for the view coordinate axes.
    up:
        Which axis is up? One of "+X", "-X", "+Y", "-Y", "+Z", "-Z".
    right_handed:
        If True, the coordinate system is right-handed. If False, it is left-handed.
    timeless:
        If true, the view coordinates will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)
    if xyz == "" and up == "":
        _send_warning("You must set either 'xyz' or 'up'. Ignoring log.", 1)
        return
    if xyz != "" and up != "":
        _send_warning("You must set either 'xyz' or 'up', but not both. Dropping up.", 1)
        up = ""
    if xyz != "":
        bindings.log_view_coordinates_xyz(
            entity_path,
            xyz,
            right_handed,
            timeless,
            recording=recording,
        )
    else:
        if right_handed is None:
            right_handed = True
        bindings.log_view_coordinates_up_handedness(
            entity_path,
            up,
            right_handed,
            timeless,
            recording=recording,
        )


@log_decorator
def log_unknown_transform(
    entity_path: str,
    timeless: bool = False,
    recording: Optional[RecordingStream] = None,
) -> None:
    """
    Log that this entity is NOT in the same space as the parent, but you do not (yet) know how they relate.

    Parameters
    ----------
    entity_path:
        The path of the affected entity.

    timeless:
        Log the data as timeless.

    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    """
    recording = RecordingStream.to_native(recording)

    instanced: Dict[str, Any] = {}
    instanced["rerun.disconnected_transform"] = DisconnectedSpaceArray.single()
    bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)


@log_decorator
def log_affine3(
    entity_path: str,
    *,
    parent_from_child: Optional[Union[TranslationAndMat3, TranslationRotationScale3D, RotationAxisAngle]] = None,
    child_from_parent: Optional[Union[TranslationAndMat3, TranslationRotationScale3D, RotationAxisAngle]] = None,
    timeless: bool = False,
    recording: Optional[RecordingStream] = None,
) -> None:
    """
    Log an affine 3D transform between this entity and the parent.

    Set either `parent_from_child` or `child_from_parent` to a tuple of `(translation_xyz, matrix3x3)`.

    The matrix is a 3x3 matrix column major matrix which can express rotation, scale and shear.

    Example
    -------
    ```
    # log scale followed by translation along the Y-axis
    rr.log_affine3(
        "world/scaled_object",
        parent_from_child=rr.TranslationRotationScale3D([0.0, 1.0, 0.0] scale=2),
    )
    # log a rotation around the z axis.
    rr.log_affine3(
        "world/z_rotated_object",
        parent_from_child=rr.RotationAxisAngle((0, 0, 1), degree=20),
    )
    ```

    Parameters
    ----------
    entity_path:
        Path of the *child* space in the space hierarchy.
    parent_from_child:
        A tuple of `(translation_xyz, matrix3x3)` mapping points in the child space to the parent space.
    child_from_parent:
        the inverse of `parent_from_child`
    timeless:
        If true, the transform will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    if parent_from_child and child_from_parent:
        raise TypeError("Set either parent_from_child or child_from_parent, but not both.")

    if parent_from_child:
        from_parent = False
        transform = parent_from_child
    elif child_from_parent:
        from_parent = True
        transform = child_from_parent

    if isinstance(transform, RotationAxisAngle):
        transform = TranslationRotationScale3D(rotation=transform)

    instanced = {"rerun.transform3d": Transform3DArray.from_transform(Transform3D(transform, from_parent))}
    bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)


@deprecated(version="0.6.0", reason="Use log_affine3 instead and, if xyz was set, log_view_coordinates")
@log_decorator
def log_rigid3(
    entity_path: str,
    *,
    parent_from_child: Optional[Tuple[npt.ArrayLike, npt.ArrayLike]] = None,
    child_from_parent: Optional[Tuple[npt.ArrayLike, npt.ArrayLike]] = None,
    xyz: str = "",
    timeless: bool = False,
    recording: Optional[RecordingStream] = None,
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
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)

    if parent_from_child and child_from_parent:
        raise TypeError("Set either parent_from_child or child_from_parent, but not both.")

    if parent_from_child:
        log_affine3(
            entity_path,
            parent_from_child=TranslationRotationScale3D(
                translation=parent_from_child[0], rotation=parent_from_child[1]
            ),
            timeless=timeless,
            recording=recording,
        )
    elif child_from_parent:
        log_affine3(
            entity_path,
            child_from_parent=TranslationRotationScale3D(
                translation=child_from_parent[0], rotation=child_from_parent[1]
            ),
            timeless=timeless,
            recording=recording,
        )
    else:
        raise TypeError("Set either parent_from_child or child_from_parent.")

    if xyz != "":
        log_view_coordinates(
            entity_path,
            xyz=xyz,
            timeless=timeless,
            recording=recording,
        )
