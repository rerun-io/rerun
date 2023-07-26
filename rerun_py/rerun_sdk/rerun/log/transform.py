"""
Methods for logging transforms on entity paths.

Learn more about transforms [in the manual](https://www.rerun.io/docs/concepts/spaces-and-transforms)
"""
from __future__ import annotations

from typing import Any

import numpy.typing as npt
from deprecated import deprecated

from rerun import bindings
from rerun.components.disconnected_space import DisconnectedSpaceArray
from rerun.components.quaternion import Quaternion
from rerun.components.transform3d import (
    Rigid3D,
    RotationAxisAngle,
    Scale3D,
    Transform3D,
    Transform3DArray,
    Translation3D,
    TranslationAndMat3,
    TranslationRotationScale3D,
)
from rerun.log.error_utils import _send_warning
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_view_coordinates",
    "log_unknown_transform",
    "log_disconnected_space",
    "log_rigid3",
    "log_transform3d",
]


@log_decorator
def log_view_coordinates(
    entity_path: str,
    *,
    xyz: str = "",
    up: str = "",
    right_handed: bool | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log the view coordinates for an entity.

    Each entity defines its own coordinate system, called a space.
    By logging view coordinates you can give semantic meaning to the XYZ axes of the space.

    This is particularly useful for 3D spaces, to set the up-axis.

    For pinhole entities this will control the direction of the camera frustum.
    You should use [`rerun.log_pinhole(…, camera_xyz=…)`][rerun.log_pinhole] for this instead,
    and read the documentation there.

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

    Currently Rerun only supports right-handed coordinate systems.

    Example
    -------
    ```
    rerun.log_view_coordinates("world/camera/image", xyz="RUB")
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


@deprecated(version="0.7.0", reason="Use log_disconnected_space instead.")
@log_decorator
def log_unknown_transform(
    entity_path: str,
    timeless: bool = False,
    recording: RecordingStream | None = None,
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

    instanced: dict[str, Any] = {}
    instanced["rerun.disconnected_space"] = DisconnectedSpaceArray.single()
    bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)


@log_decorator
def log_disconnected_space(
    entity_path: str,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log that this entity is NOT in the same space as the parent.

    This is useful for specifying that a subgraph is independent of the rest of the scene.
    If a transform or pinhole is logged on the same path, this component will be ignored.

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

    instanced: dict[str, Any] = {}
    instanced["rerun.disconnected_space"] = DisconnectedSpaceArray.single()
    bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)


@log_decorator
def log_transform3d(
    entity_path: str,
    transform: (
        TranslationAndMat3
        | TranslationRotationScale3D
        | RotationAxisAngle
        | Translation3D
        | Scale3D
        | Quaternion
        | Rigid3D
    ),
    *,
    from_parent: bool = False,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log an (affine) 3D transform between this entity and the parent.

    If `from_parent` is set to `True`, the transformation is from the parent to the space of the entity_path,
    otherwise it is from the child to the parent.

    Note that new transforms replace previous, i.e. if you call this function several times on the same path,
    each new transform will replace the previous one and does not combine with it.

    Examples
    --------
    ```
    # Log translation only.
    rr.log_transform3d("transform_test/translation", rr.Translation3D((2, 1, 3)))

    # Log scale along the x axis only.
    rr.log_transform3d("transform_test/x_scaled", rr.Scale3D((3, 1, 1)))

    # Log a rotation around the z axis.
    rr.log_transform3d("transform_test/z_rotated_object", rr.RotationAxisAngle((0, 0, 1), degrees=20))

    # Log scale followed by translation along the Y-axis.
    rr.log_transform3d(
        "transform_test/scaled_and_translated_object", rr.TranslationRotationScale3D([0.0, 1.0, 0.0], scale=2)
    )

    # Log translation + rotation, also called a rigid transform.
    rr.log_transform3d("transform_test/rigid3", rr.Rigid3D([1, 2, 3], rr.RotationAxisAngle((0, 1, 0), radians=1.57)))

    # Log translation, rotation & scale all at once.
    rr.log_transform3d(
        "transform_test/transformed",
        rr.TranslationRotationScale3D(
            translation=[0, 1, 5],
            rotation=rr.RotationAxisAngle((0, 0, 1), degrees=20),
            scale=2,
        ),
    )
    ```

    Parameters
    ----------
    entity_path:
        Path of the *child* space in the space hierarchy.
    transform:
        Instance of a rerun data class that describes a three dimensional transform.
        One of:
        * `TranslationAndMat3`
        * `TranslationRotationScale3D`
        * `Rigid3D`
        * `RotationAxisAngle`
        * `Translation3D`
        * `Quaternion`
        * `Scale3D`
    from_parent:
        If True, the transform is from the parent to the child, otherwise it is from the child to the parent.
    timeless:
        If true, the transform will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    # Convert additionally supported types to TranslationRotationScale3D
    if isinstance(transform, RotationAxisAngle) or isinstance(transform, Quaternion):
        transform = TranslationRotationScale3D(rotation=transform)
    elif isinstance(transform, Translation3D):
        transform = TranslationRotationScale3D(translation=transform)
    elif isinstance(transform, Scale3D):
        transform = TranslationRotationScale3D(scale=transform)
    elif isinstance(transform, Rigid3D):
        transform = TranslationRotationScale3D(rotation=transform.rotation, translation=transform.translation)

    instanced = {"rerun.transform3d": Transform3DArray.from_transform(Transform3D(transform, from_parent))}
    bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)


@deprecated(version="0.7.0", reason="Use log_transform3d instead and, if xyz was set, use log_view_coordinates.")
@log_decorator
def log_rigid3(
    entity_path: str,
    *,
    parent_from_child: tuple[npt.ArrayLike, npt.ArrayLike] | None = None,
    child_from_parent: tuple[npt.ArrayLike, npt.ArrayLike] | None = None,
    xyz: str = "",
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a proper rigid 3D transform between this entity and the parent (_deprecated_).

    Set either `parent_from_child` or `child_from_parent` to a tuple of `(translation_xyz, quat_xyzw)`.

    Note: This function is deprecated. Use [`rerun.log_transform3d`][] instead.

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
        rotation = None
        if parent_from_child[1] is not None:
            rotation = Quaternion(xyzw=parent_from_child[1])
        log_transform3d(
            entity_path,
            Rigid3D(translation=parent_from_child[0], rotation=rotation),
            timeless=timeless,
            recording=recording,
        )
    elif child_from_parent:
        rotation = None
        if child_from_parent[1] is not None:
            rotation = Quaternion(xyzw=child_from_parent[1])
        log_transform3d(
            entity_path,
            Rigid3D(translation=child_from_parent[0], rotation=rotation),
            from_parent=True,
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
