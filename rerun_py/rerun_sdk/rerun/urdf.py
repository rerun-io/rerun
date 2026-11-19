from __future__ import annotations

from typing import TYPE_CHECKING

from typing_extensions import deprecated

from rerun_bindings import _UrdfJointInternal, _UrdfLinkInternal, _UrdfMimicInternal, _UrdfTreeInternal

from ._kinematic_tree import _KinematicJoint, _KinematicTree

if TYPE_CHECKING:
    from pathlib import Path

    from .recording_stream import RecordingStream

__all__ = ["UrdfJoint", "UrdfLink", "UrdfMimic", "UrdfTree"]


class UrdfMimic:
    """
    A URDF `<mimic>` tag specification.

    A mimic joint's value is derived from a driver joint as
    `value = multiplier * driver_value + offset`.
    """

    def __init__(self, inner: _UrdfMimicInternal) -> None:
        self._inner = inner

    @property
    def joint(self) -> str:
        """Name of the driver joint."""
        return self._inner.joint

    @property
    def multiplier(self) -> float:
        """Multiplier applied to the driver joint's value (defaults to `1.0`)."""
        return self._inner.multiplier

    @property
    def offset(self) -> float:
        """Offset added after multiplying the driver joint's value (defaults to `0.0`)."""
        return self._inner.offset

    def __repr__(self) -> str:
        return self._inner.__repr__()


class UrdfJoint(_KinematicJoint):
    """
    A URDF joint with properties and transform computation.

    Its value is an angle in radians for revolute and continuous joints, and a distance in meters for prismatic
    joints, and is ignored for fixed joints.
    """

    _inner: _UrdfJointInternal

    @property
    def name(self) -> str:
        """Name of the joint."""
        return self._inner.name

    @property
    def joint_type(self) -> str:
        """Type of the joint (revolute, continuous, prismatic, fixed, etc.)."""
        return self._inner.joint_type

    @property
    def parent_link(self) -> str:
        """Name of the parent link."""
        return self._inner.parent_link

    @property
    def child_link(self) -> str:
        """Name of the child link."""
        return self._inner.child_link

    @property
    def axis(self) -> tuple[float, float, float]:
        """Axis of the joint."""
        return self._inner.axis

    @property
    def origin_xyz(self) -> tuple[float, float, float]:
        """Origin of the joint (translation)."""
        return self._inner.origin_xyz

    @property
    def origin_rpy(self) -> tuple[float, float, float]:
        """Origin of the joint (rotation in roll, pitch, yaw)."""
        return self._inner.origin_rpy

    @property
    def limit_lower(self) -> float:
        """Lower limit of the joint."""
        return self._inner.limit_lower

    @property
    def limit_upper(self) -> float:
        """Upper limit of the joint."""
        return self._inner.limit_upper

    @property
    def limit_effort(self) -> float:
        """Effort limit of the joint."""
        return self._inner.limit_effort

    @property
    def limit_velocity(self) -> float:
        """Velocity limit of the joint."""
        return self._inner.limit_velocity

    @property
    def mimic(self) -> UrdfMimic | None:
        """Mimic-tag specification, or `None` if this joint is not a mimic joint."""
        inner = self._inner.mimic
        return UrdfMimic(inner) if inner is not None else None


class UrdfLink:
    """A URDF link."""

    def __init__(self, inner: _UrdfLinkInternal) -> None:
        self._inner = inner

    @property
    def name(self) -> str:
        """Name of the link."""
        return self._inner.name

    def __repr__(self) -> str:
        return self._inner.__repr__()


class UrdfTree(_KinematicTree):
    """
    A URDF robot model with joints and links.

    Use [`log_to_recording`][rerun.urdf.UrdfTree.log_to_recording] to log the full model (geometry + static transforms), then animate
    individual joints by logging [`archetypes.Transform3D`][rerun.archetypes.Transform3D] computed via
    [`UrdfJoint.compute_transform`][rerun.urdf.UrdfJoint.compute_transform].
    """

    _inner: _UrdfTreeInternal

    @staticmethod
    def from_file_path(
        path: str | Path,
        entity_path_prefix: str | None = None,
        *,
        frame_prefix: str | None = None,
        static_transform_entity_path: str | None = None,
    ) -> UrdfTree:
        """
        Load a URDF file from the given path.

        Parameters
        ----------
        path:
            Path to the URDF file.
        entity_path_prefix:
            Optional entity path prefix.
        frame_prefix:
            Optional prefix for all frame IDs.
            Use to load the same URDF multiple times with unique frames.
        static_transform_entity_path:
            Optional entity path to use when logging static transforms.
            If omitted, defaults to `/tf_static`.
            This path is not affected by `entity_path_prefix`.

        """
        return UrdfTree(
            _UrdfTreeInternal.from_file_path(
                path,
                entity_path_prefix,
                frame_prefix=frame_prefix,
                static_transform_entity_path=static_transform_entity_path,
            )
        )

    def root_link(self) -> UrdfLink:
        """Get the root link of the URDF."""
        return UrdfLink(self._inner.root_link())

    def joints(self) -> list[UrdfJoint]:
        """Get all joints in the URDF."""
        return [UrdfJoint(j) for j in self._inner.joints()]

    def get_joint_by_name(self, joint_name: str) -> UrdfJoint | None:
        """
        Get a joint by name.

        Parameters
        ----------
        joint_name:
            Name of the joint.

        """
        inner = self._inner.get_joint_by_name(joint_name)
        return UrdfJoint(inner) if inner else None

    def get_joint_child(self, joint: UrdfJoint) -> UrdfLink:
        """
        Get the child link of a joint.

        Parameters
        ----------
        joint:
            The joint whose child link to retrieve.

        """
        return UrdfLink(self._inner.get_joint_child(joint._inner))

    def get_link_by_name(self, link_name: str) -> UrdfLink | None:
        """
        Get a link by name.

        Parameters
        ----------
        link_name:
            Name of the link.

        """
        inner = self._inner.get_link_by_name(link_name)
        return UrdfLink(inner) if inner else None

    def get_visual_geometry_paths(self, link: str | UrdfLink) -> list[str]:
        """
        Get the entity paths for all visual geometries of the given link, if any.

        Parameters
        ----------
        link:
            The link for which to get visual geometry paths,
            either by name or as an UrdfLink instance.

        """
        inner = self._link_inner(link)
        return self._inner.get_visual_geometry_paths(inner) if inner is not None else []

    def get_collision_geometry_paths(self, link: str | UrdfLink) -> list[str]:
        """
        Get the entity paths for all collision geometries of the given link, if any.

        Parameters
        ----------
        link:
            The link for which to retrieve the collision geometry entity paths,
            either by name or as an UrdfLink instance.

        """
        inner = self._link_inner(link)
        return self._inner.get_collision_geometry_paths(inner) if inner is not None else []

    def _link_inner(self, link: str | UrdfLink) -> _UrdfLinkInternal | None:
        if isinstance(link, str):
            return self._inner.get_link_by_name(link)
        return link._inner

    @deprecated("Renamed to `log_to_recording`.")
    def log_urdf_to_recording(self, recording: RecordingStream | None = None) -> None:
        """
        Log the full robot model (geometry + static transforms) to a recording stream.

        Parameters
        ----------
        recording:
            The recording stream to log to. If `None`, the current active recording is used.

        """
        self.log_to_recording(recording)
