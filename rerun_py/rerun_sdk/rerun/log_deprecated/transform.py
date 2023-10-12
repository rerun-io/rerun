"""
Methods for logging transforms on entity paths.

Learn more about transforms [in the manual](https://www.rerun.io/docs/concepts/spaces-and-transforms)
"""
from __future__ import annotations

from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.archetypes import DisconnectedSpace, ViewCoordinates
from rerun.datatypes import (
    Quaternion,
    RotationAxisAngle,
    Scale3D,
    TranslationAndMat3x3,
    TranslationRotationScale3D,
    Vec3D,
)
from rerun.error_utils import _send_warning_or_raise
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

# Legacy alias for `TranslationAndMat3x3`
TranslationAndMat3 = TranslationAndMat3x3
"""
!!! Warning "Deprecated"
    Please migrate to [rerun.log][] with [rerun.TranslationAndMat3x3][].

    See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.
"""

# Legacy alias for `TranslationRotationScale3D`
Rigid3D = TranslationRotationScale3D
"""
!!! Warning "Deprecated"
    Please migrate to [rerun.log][] with [rerun.TranslationRotationScale3D][].

    See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.
"""

# Legacy alias for `Vec3D`
Translation3D = Vec3D
"""
!!! Warning "Deprecated"
    Please migrate to [rerun.log][] with [rerun.datatypes.Vec3D][].

    See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.
"""


__all__ = [
    "log_view_coordinates",
    "log_disconnected_space",
    "log_transform3d",
]

_up_attrs = {
    ("+X", True): "RIGHT_HAND_X_UP",
    ("+X", False): "LEFT_HAND_X_UP",
    ("-X", True): "RIGHT_HAND_X_DOWN",
    ("-X", False): "LEFT_HAND_X_DOWN",
    ("+Y", True): "RIGHT_HAND_Y_UP",
    ("+Y", False): "LEFT_HAND_Y_UP",
    ("-Y", True): "RIGHT_HAND_Y_DOWN",
    ("-Y", False): "LEFT_HAND_Y_DOWN",
    ("+Z", True): "RIGHT_HAND_Z_UP",
    ("+Z", False): "LEFT_HAND_Z_UP",
    ("-Z", True): "RIGHT_HAND_Z_DOWN",
    ("-Z", False): "LEFT_HAND_Z_DOWN",
}


@deprecated(
    """Please migrate to `rr.log(…, rr.ViewCoordinates(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
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

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.ViewCoordinates][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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
        _send_warning_or_raise("You must set either 'xyz' or 'up'. Ignoring log.", 1)
        return
    if xyz != "" and up != "":
        _send_warning_or_raise("You must set either 'xyz' or 'up', but not both. Dropping up.", 1)
        up = ""
    if xyz != "":
        xyz = xyz.upper()
        if hasattr(ViewCoordinates, xyz):
            log(entity_path, getattr(ViewCoordinates, xyz), timeless=timeless, recording=recording)
        else:
            raise ValueError(f"Could not interpret xyz={xyz} as a valid ViewDirection.")
    else:
        if right_handed is None:
            right_handed = True

        if (up.upper(), right_handed) not in _up_attrs:
            raise ValueError(f"Could not interpret up={up} as a valid ViewDirection.")

        up = _up_attrs[(up, right_handed)]

        if hasattr(ViewCoordinates, up):
            log(entity_path, getattr(ViewCoordinates, up), timeless=timeless, recording=recording)
        else:
            # This should never get hit
            raise ValueError("Invalid up value.")


@deprecated(
    """Please migrate to `rr.log(…, rr.DisconnectedSpace(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_disconnected_space(
    entity_path: str,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log that this entity is NOT in the same space as the parent.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.DisconnectedSpace][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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

    log(entity_path, DisconnectedSpace(), timeless=timeless, recording=recording)


@deprecated(
    """Please migrate to `rr.log(…, rr.Transform3D(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
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

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Transform3D][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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
    from rerun._log import log
    from rerun.archetypes import Transform3D

    new_transform: TranslationRotationScale3D | TranslationAndMat3x3 | None = None

    if isinstance(transform, RotationAxisAngle):
        rotation = transform
        new_transform = TranslationRotationScale3D(rotation=rotation, from_parent=from_parent)
    elif isinstance(transform, Quaternion):
        quat = transform
        new_transform = TranslationRotationScale3D(rotation=quat, from_parent=from_parent)
    elif isinstance(transform, Translation3D):
        translation = transform
        new_transform = TranslationRotationScale3D(translation=translation, from_parent=from_parent)
    elif isinstance(transform, Scale3D):
        scale = transform
        new_transform = TranslationRotationScale3D(scale=scale, from_parent=from_parent)
    elif isinstance(transform, (Rigid3D, TranslationRotationScale3D, TranslationAndMat3)):
        new_transform = transform
        new_transform.from_parent = from_parent
    else:
        raise ValueError("Invalid transform type.")

    log(entity_path, Transform3D(new_transform), timeless=timeless, recording=recording)
