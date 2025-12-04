from __future__ import annotations


import warnings
from typing import TYPE_CHECKING

from rerun_bindings import UrdfJoint as _UrdfJoint
from rerun_bindings import UrdfLink as _UrdfLink
from rerun_bindings import UrdfTree as _UrdfTree

if TYPE_CHECKING:
    from pathlib import Path

    from . import Transform3D

__all__ = ["UrdfJoint", "UrdfLink", "UrdfTree"]


class UrdfJoint:
    """
    Wrapper for URDF joint with utility methods.

    This class wraps the Rust binding and provides additional utilities
    for working with URDF joints in Rerun.
    """

    def __init__(self, inner: _UrdfJoint) -> None:
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

    def compute_transform(self, angle: float) -> Transform3D:
        """
        Compute a Transform3D archetype for the given joint angle.

        This computes the dynamic transform contribution from joint motion.
        The joint's static origin transform is NOT included (it should be logged
        separately with the URDF file).

        Parameters
        ----------
        angle : float
            Joint angle in radians. For revolute/continuous joints, this is the rotation angle.
            For prismatic joints, this is the translation distance.
            For fixed joints, this parameter is ignored.

        Returns
        -------
        Transform3D
            A Transform3D archetype ready to log to Rerun.

        Raises
        ------
        NotImplementedError
            For unsupported joint types (floating, planar, spherical).

        Warns
        -----
        UserWarning
            If angle exceeds joint limits (value is clamped automatically).

        Examples
        --------
        >>> import rerun as rr
        >>> tree = rr.UrdfTree.from_file_path("robot.urdf")
        >>> joint = tree.get_joint_by_name("shoulder_pan")
        >>> transform = joint.compute_transform(1.57)  # ~90 degrees
        >>> link = tree.get_joint_child(joint)
        >>> path = tree.get_link_path(link)
        >>> rr.log(path, transform)

        """
        from . import Transform3D
        from .datatypes import RotationAxisAngle

        jtype = self.joint_type

        if jtype in ("revolute", "continuous"):
            # Revolute and continuous joints rotate around their axis
            if jtype == "revolute":
                # Check limits only for revolute (continuous has no limits)
                if not (self.limit_lower <= angle <= self.limit_upper):
                    warnings.warn(
                        f"Joint '{self.name}' angle {angle:.4f} rad is outside limits "
                        f"[{self.limit_lower:.4f}, {self.limit_upper:.4f}] rad. Clamping.",
                        UserWarning,
                        stacklevel=2,
                    )
                    angle = max(self.limit_lower, min(self.limit_upper, angle))

            return Transform3D(
                rotation_axis_angle=RotationAxisAngle(
                    axis=self.axis,
                    radians=angle,
                )
            )

        elif jtype == "prismatic":
            # Prismatic joints translate along their axis
            if not (self.limit_lower <= angle <= self.limit_upper):
                warnings.warn(
                    f"Joint '{self.name}' distance {angle:.4f} m is outside limits "
                    f"[{self.limit_lower:.4f}, {self.limit_upper:.4f}] m. Clamping.",
                    UserWarning,
                    stacklevel=2,
                )
                angle = max(self.limit_lower, min(self.limit_upper, angle))

            # Translate along joint axis
            axis_x, axis_y, axis_z = self.axis
            translation = (axis_x * angle, axis_y * angle, axis_z * angle)
            return Transform3D(translation=translation)

        elif jtype == "fixed":
            # Fixed joints have no motion - return identity transform
            return Transform3D()

        else:
            # Unsupported joint types
            raise NotImplementedError(
                f"Joint type '{jtype}' is not supported by compute_transform(). "
                f"Supported types are: revolute, continuous, prismatic, fixed. "
                f"Unsupported types (floating, planar, spherical) require advanced kinematics."
            )

    def __repr__(self) -> str:
        return self._inner.__repr__()


class UrdfLink:
    """
    Wrapper for URDF link.

    This class wraps the Rust binding and provides a consistent API
    for working with URDF links in Rerun.
    """

    def __init__(self, inner: _UrdfLink) -> None:
        self._inner = inner

    @property
    def name(self) -> str:
        """Name of the link."""
        return self._inner.name

    def __repr__(self) -> str:
        return self._inner.__repr__()


class UrdfTree:
    """
    Wrapper for URDF tree with utility methods.

    This class wraps the Rust binding and provides wrapped types
    for all methods that return joints or links.
    """

    def __init__(self, inner: _UrdfTree) -> None:
        self._inner = inner

    @staticmethod
    def from_file_path(path: str | Path) -> UrdfTree:
        """
        Load the URDF found at `path`.

        Parameters
        ----------
        path : str | Path
            Path to the URDF file to load.

        Returns
        -------
        UrdfTree
            The loaded URDF tree.

        """
        return UrdfTree(_UrdfTree.from_file_path(path))

    @property
    def name(self) -> str:
        """Name of the robot defined in this URDF."""
        return self._inner.name

    def root_link(self) -> UrdfLink:
        """
        Returns the root link of the URDF hierarchy.

        Returns
        -------
        UrdfLink
            The root link of the URDF.

        """
        return UrdfLink(self._inner.root_link())

    def joints(self) -> list[UrdfJoint]:
        """
        Iterate over all joints defined in the URDF.

        Returns
        -------
        list[UrdfJoint]
            List of all joints in the URDF.

        """
        return [UrdfJoint(j) for j in self._inner.joints()]

    def get_joint_by_name(self, joint_name: str) -> UrdfJoint | None:
        """
        Find a joint by name.

        Parameters
        ----------
        joint_name : str
            Name of the joint to find.

        Returns
        -------
        UrdfJoint | None
            The joint with the given name, or None if not found.

        """
        inner = self._inner.get_joint_by_name(joint_name)
        return UrdfJoint(inner) if inner else None

    def get_joint_child(self, joint: UrdfJoint) -> UrdfLink:
        """
        Returns the link that is the child of the given joint.

        Parameters
        ----------
        joint : UrdfJoint
            The joint whose child link to retrieve.

        Returns
        -------
        UrdfLink
            The child link of the joint.

        """
        return UrdfLink(self._inner.get_joint_child(joint._inner))

    def get_link_by_name(self, link_name: str) -> UrdfLink | None:
        """
        Returns the link with the given name, if it exists.

        Parameters
        ----------
        link_name : str
            Name of the link to find.

        Returns
        -------
        UrdfLink | None
            The link with the given name, or None if not found.

        """
        inner = self._inner.get_link_by_name(link_name)
        return UrdfLink(inner) if inner else None

    def get_link_path(self, link: UrdfLink) -> str:
        """
        Returns the entity path assigned to the given link.

        Parameters
        ----------
        link : UrdfLink
            The link whose path to retrieve.

        Returns
        -------
        str
            The entity path for the link.

        """
        return self._inner.get_link_path(link._inner)

    def get_link_path_by_name(self, link_name: str) -> str | None:
        """
        Returns the entity path for the named link, if it exists.

        Parameters
        ----------
        link_name : str
            Name of the link whose path to retrieve.

        Returns
        -------
        str | None
            The entity path for the link, or None if the link doesn't exist.

        """
        return self._inner.get_link_path_by_name(link_name)

    def __repr__(self) -> str:
        return self._inner.__repr__()
