from __future__ import annotations

import math
import warnings
from typing import TYPE_CHECKING

from rerun_bindings import _UrdfJointInternal, _UrdfLinkInternal, _UrdfTreeInternal

if TYPE_CHECKING:
    from pathlib import Path

    from . import Transform3D

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

    def compute_transform(self, angle: float) -> Transform3D:
        """
        Compute a Transform3D for this joint at the given angle.

        Parameters
        ----------
        angle:
            Joint angle in radians (revolute/continuous) or distance in meters (prismatic).
            Ignored for fixed joints. Values outside limits are clamped with a warning.

        Returns
        -------
        Transform3D
            Transform with rotation, translation, parent_frame, and child_frame ready to log.

        """
        from . import Transform3D
        from .datatypes import Quaternion

        joint_type = self.joint_type

        if joint_type in ("revolute", "continuous"):
            # Revolute and continuous joints rotate around their axis
            # Check limits only for revolute (continuous has no limits)
            if joint_type == "revolute" and not (self.limit_lower <= angle <= self.limit_upper):
                warnings.warn(
                    f"Joint '{self.name}' angle {angle:.4f} rad is outside limits "
                    f"[{self.limit_lower:.4f}, {self.limit_upper:.4f}] rad. Clamping.",
                    UserWarning,
                    stacklevel=2,
                )
                angle = max(self.limit_lower, min(self.limit_upper, angle))

            # Combine origin rotation (RPY) with dynamic rotation (axis-angle)
            # First convert origin RPY to quaternion
            roll, pitch, yaw = self.origin_rpy
            quat_origin = _euler_to_quat(roll, pitch, yaw)

            # Convert axis-angle to quaternion
            axis_x, axis_y, axis_z = self.axis
            half_angle = angle / 2.0
            sin_half = math.sin(half_angle)
            cos_half = math.cos(half_angle)
            quat_dynamic = [
                axis_x * sin_half,  # x
                axis_y * sin_half,  # y
                axis_z * sin_half,  # z
                cos_half,  # w
            ]

            # Multiply quaternions: quat_origin * quat_dynamic
            combined_quat = _quat_multiply(quat_origin, quat_dynamic)

            return Transform3D(
                quaternion=Quaternion(xyzw=combined_quat),
                translation=self.origin_xyz,
                parent_frame=self.parent_link,
                child_frame=self.child_link,
            )

        elif joint_type == "prismatic":
            # Prismatic joints translate along their axis
            if not (self.limit_lower <= angle <= self.limit_upper):
                warnings.warn(
                    f"Joint '{self.name}' distance {angle:.4f} m is outside limits "
                    f"[{self.limit_lower:.4f}, {self.limit_upper:.4f}] m. Clamping.",
                    UserWarning,
                    stacklevel=2,
                )
                angle = max(self.limit_lower, min(self.limit_upper, angle))

            # Compute translation: origin + dynamic offset along axis
            axis_x, axis_y, axis_z = self.axis
            origin_x, origin_y, origin_z = self.origin_xyz
            translation = (
                origin_x + axis_x * angle,
                origin_y + axis_y * angle,
                origin_z + axis_z * angle,
            )

            # For prismatic joints, rotation is just the origin rotation
            roll, pitch, yaw = self.origin_rpy
            quat = None
            if roll != 0.0 or pitch != 0.0 or yaw != 0.0:
                quat = Quaternion(xyzw=_euler_to_quat(roll, pitch, yaw))
            return Transform3D(
                translation=translation,
                quaternion=quat,
                parent_frame=self.parent_link,
                child_frame=self.child_link,
            )

        elif joint_type == "fixed":
            # Fixed joints only have the origin transform
            roll, pitch, yaw = self.origin_rpy
            quat = None
            if roll != 0.0 or pitch != 0.0 or yaw != 0.0:
                quat = Quaternion(xyzw=_euler_to_quat(roll, pitch, yaw))
            return Transform3D(
                translation=self.origin_xyz,
                quaternion=quat,
                parent_frame=self.parent_link,
                child_frame=self.child_link,
            )

        else:
            # Unsupported joint types
            raise NotImplementedError(
                f"Joint type '{joint_type}' is not supported by compute_transform(). "
                f"Supported types are: revolute, continuous, prismatic, fixed. "
                f"Unsupported types (floating, planar, spherical) require advanced kinematics."
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
    def from_file_path(path: str | Path) -> UrdfTree:
        """
        Load a URDF file from the given path.

        Parameters
        ----------
        path:
            Path to the URDF file.

        """
        return UrdfTree(_UrdfTreeInternal.from_file_path(path))

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

    def get_link_path(self, link: UrdfLink) -> str:
        """
        Get the entity path for a link.

        Parameters
        ----------
        link:
            The link whose path to retrieve.

        """
        return self._inner.get_link_path(link._inner)

    def get_link_path_by_name(self, link_name: str) -> str | None:
        """
        Get the entity path for a link by name.

        Parameters
        ----------
        link_name:
            Name of the link.

        """
        return self._inner.get_link_path_by_name(link_name)

    def __repr__(self) -> str:
        return self._inner.__repr__()


def _euler_to_quat(roll: float, pitch: float, yaw: float) -> list[float]:
    """Convert Euler angles (RPY) to quaternion (XYZW)."""
    cr = math.cos(roll * 0.5)
    sr = math.sin(roll * 0.5)
    cp = math.cos(pitch * 0.5)
    sp = math.sin(pitch * 0.5)
    cy = math.cos(yaw * 0.5)
    sy = math.sin(yaw * 0.5)

    w = cr * cp * cy + sr * sp * sy
    x = sr * cp * cy - cr * sp * sy
    y = cr * sp * cy + sr * cp * sy
    z = cr * cp * sy - sr * sp * cy

    return [x, y, z, w]


def _quat_multiply(q1: list[float], q2: list[float]) -> list[float]:
    """Multiply two quaternions in XYZW format."""
    x1, y1, z1, w1 = q1
    x2, y2, z2, w2 = q2

    w = w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2
    x = w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2
    y = w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2
    z = w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2

    return [x, y, z, w]
