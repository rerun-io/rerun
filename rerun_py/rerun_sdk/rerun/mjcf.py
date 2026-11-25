from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import _MjcfBodyInternal, _MjcfJointInternal, _MjcfTreeInternal

from ._kinematic_tree import _KinematicJoint, _KinematicTree

if TYPE_CHECKING:
    from pathlib import Path

__all__ = ["MjcfBody", "MjcfJoint", "MjcfTree"]


class MjcfJoint(_KinematicJoint):
    """
    An MJCF joint with properties and transform computation.

    A joint moves its body relative to the body's parent.
    A body can have several joints, which MuJoCo applies in order.
    Each of them moves a frame of its own: a body `b` with joints `j1`, `j2` and `j3` has the intermediate frames
    `b::j1` and `b::j2`, so that the transforms of its joints chain up from the parent body to `b`.

    Its value is an angle in radians for hinge joints, and a distance in meters for slide joints, like MuJoCo's `qpos`.
    """

    _inner: _MjcfJointInternal

    @property
    def name(self) -> str:
        """Name of the joint. Unnamed joints are called `joint_<id>`."""
        return self._inner.name

    @property
    def joint_type(self) -> str:
        """Type of the joint (hinge, slide, ball or free)."""
        return self._inner.joint_type

    @property
    def body(self) -> str:
        """Name of the body this joint moves."""
        return self._inner.body

    @property
    def parent_body(self) -> str | None:
        """Name of the parent of the body this joint moves."""
        return self._inner.parent_body

    @property
    def axis(self) -> tuple[float, float, float]:
        """Unit axis of the joint, in the body's frame."""
        return self._inner.axis

    @property
    def anchor(self) -> tuple[float, float, float]:
        """The point the joint rotates about, in the body's frame."""
        return self._inner.anchor

    @property
    def reference(self) -> float:
        """The value at which the body is in its reference pose (MJCF `ref`)."""
        return self._inner.reference

    @property
    def range(self) -> tuple[float, float] | None:
        """The lower and upper limit, or `None` if the joint is not limited."""
        return self._inner.range


class MjcfBody:
    """An MJCF body, which has a frame named after it."""

    def __init__(self, inner: _MjcfBodyInternal) -> None:
        self._inner = inner

    @property
    def name(self) -> str:
        """Name of the body. Unnamed bodies are called `body_<id>`, and the root is called `world`."""
        return self._inner.name

    @property
    def parent(self) -> str | None:
        """Name of the parent body, or `None` for the world body."""
        return self._inner.parent

    @property
    def translation(self) -> tuple[float, float, float]:
        """Translation relative to the parent body, when all joints are at their reference value."""
        return self._inner.translation

    @property
    def quaternion_xyzw(self) -> tuple[float, float, float, float]:
        """
        Rotation relative to the parent body, when all joints are at their reference value.

        A unit quaternion in `xyzw` order, as [`Quaternion`][rerun.encodings.Quaternion] takes it, unlike MuJoCo's `wxyz`.
        """
        return self._inner.quaternion_xyzw

    def joints(self) -> list[MjcfJoint]:
        """The joints that move this body relative to its parent, in the order MuJoCo applies them."""
        return [MjcfJoint(j) for j in self._inner.joints()]

    def __repr__(self) -> str:
        return self._inner.__repr__()


class MjcfTree(_KinematicTree):
    """
    An [MJCF](https://mujoco.readthedocs.io/en/stable/XMLreference.html) model with bodies and joints.

    The model's name is its `model` attribute, or else its file name.

    Use [`log_to_recording`][rerun.mjcf.MjcfTree.log_to_recording] to log the full model (geometry + static transforms), then animate
    individual joints by logging [`archetypes.Transform3D`][rerun.archetypes.Transform3D] computed via
    [`MjcfJoint.compute_transform`][rerun.mjcf.MjcfJoint.compute_transform].
    """

    _inner: _MjcfTreeInternal

    @staticmethod
    def from_file_path(
        path: str | Path,
        entity_path_prefix: str | None = None,
        *,
        frame_prefix: str | None = None,
        static_transform_entity_path: str | None = None,
    ) -> MjcfTree:
        """
        Load an MJCF file (`.mjcf` or `.xml`) from the given path, along with the files it includes.

        Parameters
        ----------
        path:
            Path to the MJCF file.
        entity_path_prefix:
            Optional entity path prefix.
        frame_prefix:
            Optional prefix for all frame IDs.
            Use to load the same model multiple times with unique frames.
        static_transform_entity_path:
            Optional entity path to use when logging static transforms.
            If omitted, defaults to `/tf_static`.
            This path is not affected by `entity_path_prefix`.

        """
        return MjcfTree(
            _MjcfTreeInternal.from_file_path(
                path,
                entity_path_prefix,
                frame_prefix=frame_prefix,
                static_transform_entity_path=static_transform_entity_path,
            )
        )

    def root_body(self) -> MjcfBody:
        """Get the world body, which is the root of the body tree."""
        return MjcfBody(self._inner.root_body())

    def bodies(self) -> list[MjcfBody]:
        """Get all bodies, parents before children."""
        return [MjcfBody(b) for b in self._inner.bodies()]

    def get_body_by_name(self, body_name: str) -> MjcfBody | None:
        """
        Get a body by name.

        Parameters
        ----------
        body_name:
            Name of the body.

        """
        inner = self._inner.get_body_by_name(body_name)
        return MjcfBody(inner) if inner else None

    def joints(self) -> list[MjcfJoint]:
        """Get all joints in the model."""
        return [MjcfJoint(j) for j in self._inner.joints()]

    def get_joint_by_name(self, joint_name: str) -> MjcfJoint | None:
        """
        Get a joint by name.

        Parameters
        ----------
        joint_name:
            Name of the joint.

        """
        inner = self._inner.get_joint_by_name(joint_name)
        return MjcfJoint(inner) if inner else None

    def get_joint_body(self, joint: MjcfJoint) -> MjcfBody:
        """
        Get the body that a joint moves.

        Parameters
        ----------
        joint:
            The joint whose body to retrieve.

        """
        body = self.get_body_by_name(joint.body)
        if body is None:
            raise ValueError(f"Joint {joint.name!r} is not part of MJCF model {self.name!r}")
        return body

    def get_visual_geometry_paths(self, body: str | MjcfBody) -> list[str]:
        """
        Get the entity paths for all visual geometries of the given body, if any.

        Parameters
        ----------
        body:
            The body for which to get visual geometry paths,
            either by name or as an MjcfBody instance.

        """
        inner = self._body_inner(body)
        return self._inner.get_visual_geometry_paths(inner) if inner is not None else []

    def get_collision_geometry_paths(self, body: str | MjcfBody) -> list[str]:
        """
        Get the entity paths for all collision geometries of the given body, if any.

        Parameters
        ----------
        body:
            The body for which to get collision geometry paths,
            either by name or as an MjcfBody instance.

        """
        inner = self._body_inner(body)
        return self._inner.get_collision_geometry_paths(inner) if inner is not None else []

    def _body_inner(self, body: str | MjcfBody) -> _MjcfBodyInternal | None:
        if isinstance(body, str):
            return self._inner.get_body_by_name(body)
        return body._inner
