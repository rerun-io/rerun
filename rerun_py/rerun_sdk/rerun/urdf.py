from __future__ import annotations

import warnings
from typing import TYPE_CHECKING

from rerun_bindings import _UrdfJointInternal, _UrdfLinkInternal, _UrdfTreeInternal

if TYPE_CHECKING:
    from collections.abc import Sequence
    from pathlib import Path

    from . import Transform3D
    from ._baseclasses import ComponentColumnList

__all__ = ["UrdfJoint", "UrdfLink", "UrdfTree"]


class UrdfJoint:
    """A URDF joint with properties and transform computation."""

    def __init__(self, inner: _UrdfJointInternal) -> None:
        self._inner = inner

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

    def compute_transform(self, value: float, clamp: bool = True) -> Transform3D:
        """
        Compute a Transform3D for this joint at the given value.

        Parameters
        ----------
        value:
            Joint angle in radians (revolute/continuous) or distance in meters (prismatic).
            Ignored for fixed joints. Values outside limits are clamped with a warning if `clamp` is True.
        clamp:
            Whether to clamp & warn about values outside joint limits.

        Returns
        -------
        Transform3D
            Transform with rotation, translation, parent_frame, and child_frame ready to log.

        """
        from . import Transform3D
        from .datatypes import Quaternion

        result = self._inner.compute_transform(value, clamp=clamp)

        if result["warning"] is not None:
            warnings.warn(result["warning"], UserWarning, stacklevel=2)

        return Transform3D(
            quaternion=Quaternion(xyzw=result["quaternion_xyzw"]),
            translation=result["translation"],
            parent_frame=result["parent_frame"],
            child_frame=result["child_frame"],
        )

    def compute_transform_columns(self, values: Sequence[float], clamp: bool = True) -> ComponentColumnList:
        """
        Compute transforms for this joint at multiple values, returning columnar data for use with `send_columns`.

        Parameters
        ----------
        values:
            Joint values: angles in radians (revolute/continuous) or distances in meters (prismatic).
            Values outside limits are clamped with a warning if `clamp` is True.
        clamp:
            Whether to clamp & warn about values outside joint limits.

        Returns
        -------
        ComponentColumnList
            Columnar transform data ready for use with :func:`rerun.send_columns`.

        """
        from . import Transform3D

        result = self._inner.compute_transform_columns(list(values), clamp=clamp)

        for warning in result["warnings"]:
            warnings.warn(warning, UserWarning, stacklevel=2)

        return Transform3D.columns(
            translation=result["translations"],
            quaternion=result["quaternions_xyzw"],
            parent_frame=[result["parent_frame"]] * len(values),
            child_frame=[result["child_frame"]] * len(values),
        )

    def __repr__(self) -> str:
        return self._inner.__repr__()


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


class UrdfTree:
    """
    A URDF robot model with joints and links.

    Not directly loggable. Use this to access the structure of a URDF file
    and compute transforms for individual joints, which can then be logged
    using [`archetypes.Transform3D`][rerun.archetypes.Transform3D].
    """

    def __init__(self, inner: _UrdfTreeInternal) -> None:
        self._inner = inner

    @staticmethod
    def from_file_path(path: str | Path, entity_path_prefix: str | None = None) -> UrdfTree:
        """
        Load a URDF file from the given path.

        Parameters
        ----------
        path:
            Path to the URDF file.
        entity_path_prefix:
            Optional entity path prefix.

        """
        return UrdfTree(_UrdfTreeInternal.from_file_path(path, entity_path_prefix))

    @property
    def name(self) -> str:
        """Name of the robot defined in this URDF."""
        return self._inner.name

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
        if isinstance(link, str):
            inner = self._inner.get_link_by_name(link)
            if inner is None:
                return []
        else:
            inner = link._inner
        return self._inner.get_visual_geometry_paths(inner)

    def get_collision_geometry_paths(self, link: str | UrdfLink) -> list[str]:
        """
        Get the entity paths for all collision geometries of the given link, if any.

        Parameters
        ----------
        link:
            The link for which to retrieve the collision geometry entity paths,
            either by name or as an UrdfLink instance.

        """
        if isinstance(link, str):
            inner = self._inner.get_link_by_name(link)
            if inner is None:
                return []
        else:
            inner = link._inner
        return self._inner.get_collision_geometry_paths(inner)

    def __repr__(self) -> str:
        return self._inner.__repr__()
