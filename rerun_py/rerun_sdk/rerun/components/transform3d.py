from __future__ import annotations

from dataclasses import dataclass

import numpy.typing as npt

from rerun.components.quaternion import Quaternion

__all__ = [
    "Quaternion",
    "Rigid3D",
    "RotationAxisAngle",
    "Scale3D",
    "Transform3D",
    "Translation3D",
    "TranslationAndMat3",
    "TranslationRotationScale3D",
]


@dataclass
class Transform3D:
    """An affine transform between two 3D spaces, represented in a given direction."""

    transform: TranslationAndMat3 | TranslationRotationScale3D
    """Representation of a 3D transform."""

    from_parent: bool = False
    """
    If True, the transform maps from the parent space to the child space.
    Otherwise, the transform maps from the child space to the parent space.
    """


@dataclass
class TranslationAndMat3:
    """Representation of a affine transform via a 3x3 translation matrix paired with a translation."""

    translation: npt.ArrayLike | Translation3D | None = None
    """3D translation vector, applied after the matrix. Uses (0, 0, 0) if not set."""

    matrix: npt.ArrayLike | None = None
    """The row-major 3x3 matrix for scale, rotation & skew matrix. Uses identity if not set."""


@dataclass
class Rigid3D:
    """Representation of a rigid transform via separate translation & rotation."""

    translation: Translation3D | npt.ArrayLike | None = None
    """3D translation vector, applied last."""

    rotation: Quaternion | RotationAxisAngle | None = None
    """3D rotation, represented as a quaternion or axis + angle, applied second."""


@dataclass
class TranslationRotationScale3D:
    """Representation of an affine transform via separate translation, rotation & scale."""

    translation: Translation3D | npt.ArrayLike | None = None
    """3D translation vector, applied last."""

    rotation: Quaternion | RotationAxisAngle | None = None
    """3D rotation, represented as a quaternion or axis + angle, applied second."""

    scale: Scale3D | npt.ArrayLike | float | None = None
    """3D scaling either a 3D vector, scalar or None. Applied first."""


@dataclass
class Translation3D:
    """3D translation expressed as a vector."""

    translation: npt.ArrayLike


@dataclass
class Scale3D:
    """3D scale expressed as either a uniform scale or a vector."""

    scale: npt.ArrayLike | float


@dataclass
class RotationAxisAngle:
    """3D rotation expressed via a rotation axis and angle."""

    axis: npt.ArrayLike
    """
    Axis to rotate around.

    This is not required to be normalized.
    If normalization fails (typically because the vector is length zero), the rotation is silently ignored.
    """

    degrees: float | None = None
    """3D rotation angle in degrees. Only one of `degrees` or `radians` should be set."""

    radians: float | None = None
    """3D rotation angle in radians. Only one of `degrees` or `radians` should be set."""
