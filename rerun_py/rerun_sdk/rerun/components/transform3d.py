from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Sequence

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory, build_dense_union

__all__ = [
    "Transform3DArray",
    "Transform3DType",
]


@dataclass
class UnknownTransform:
    """
    We don't know the transform, but it is likely/potentially non-identity.
    Maybe the user intend to set the transform later.
    """


@dataclass
class Pinhole:
    """Camera perspective projection (a.k.a. intrinsics)."""

    # Column-major projection matrix.
    #
    # Child from parent.
    # Image coordinates from camera view coordinates.
    image_from_cam: npt.ArrayLike

    # Pixel resolution (usually integers) of child image space. Width and height.
    resolution: npt.ArrayLike | None


@dataclass
class DirectedAffine3D:
    """An affine transform between two 3D spaces."""

    affine3d: TranslationMatrix3x3 | TranslationRotationScale3D
    """Representation of an Affine3D transform."""

    direction: TransformDirection
    """The direction of the transform."""


@dataclass
class TransformDirection(Enum):
    """Direction of a transform."""

    ChildFromParent = "ChildFromParent"
    """The transform maps from the parent space to the child space."""

    ParentFromChild = "ParentFromChild"
    """The transform maps from the child space to the parent space."""


@dataclass
class TranslationMatrix3x3:
    """Representation of a affine transform via a 3x3 translation matrix paired with a translation."""

    translation: npt.ArrayLike
    """3D translation vector, applied after the matrix."""

    matrix: npt.ArrayLike
    """The column-major 3x3 matrix for scale, rotation & skew matrix."""


@dataclass
class TranslationRotationScale3D:
    """Representation of an affine transform via separate translation, rotation & scale."""

    translation: Sequence[float] | None
    """3D translation vector, applied last."""

    rotation: Sequence[float] | AxisAngleRotation3D | None
    """3D rotation, represented as a (xyzw) quaternion or axis + angle, applied second."""

    scale: Sequence[float] | None
    """3D scaling scalar or 3D vector (may be none), applied first."""


@dataclass
class AxisAngleRotation3D:
    """3D rotation expressed via a rotation axis and angle."""

    axis: Sequence[float]
    """3D rotation axis."""

    angle: float
    """3D rotation angle, either in radians or degree."""

    is_degree: bool
    """Whether the angle is in degree or radians, (None implies radian)."""


class Transform3DArray(pa.ExtensionArray):  # type: ignore[misc]
    def affine3d_from_numpy(transform: Sequence[npt.ArrayLike, npt.ArrayLike], direction: TransformDirection):
        """Build a `Transform3DArray` with a single element from numpy arrays."""

        TranslationMatrix3x3(transform[0], transform[1])
        return Transform3DArray.from_transform()

    def from_transform(transform: DirectedAffine3D | UnknownTransform | Pinhole) -> Transform3DArray:
        """Build a `Transform3DArray` from a single transform."""

        if isinstance(transform, UnknownTransform):
            discriminant_transform3d = "Unknown"
            transform3d = pa.array([False])
        elif isinstance(transform, DirectedAffine3D):
            discriminant_transform3d = "Affine3D"
            discriminant_affine3d_direction = transform.direction.name

            directed_affine3d_union_type = Transform3DType.storage_type[1]
            affine3d_union_type = directed_affine3d_union_type.type[0]  # both [0] and [1] are the same type

            if isinstance(transform.affine3d, TranslationMatrix3x3):
                translation_matrix3x3_type = affine3d_union_type.type[0]
                discriminant_affine3d = translation_matrix3x3_type.name

                translation = np.array(transform.affine3d.translation, dtype=np.float32).flatten()
                matrix = np.array(transform.affine3d.matrix, dtype=np.float32).flatten()
                affine3d = pa.StructArray.from_arrays(
                    [
                        pa.FixedSizeListArray.from_arrays(
                            translation, type=translation_matrix3x3_type.type["translation"].type
                        ),
                        pa.FixedSizeListArray.from_arrays(matrix, type=translation_matrix3x3_type.type["matrix"].type),
                    ],
                    fields=list(translation_matrix3x3_type.type),
                )
            elif isinstance(transform.affine3d, TranslationRotationScale3D):
                translation_matrix3x3_type = affine3d_union_type.type[1]
                discriminant_affine3d = translation_matrix3x3_type.name

                discriminant_affine3d = "TranslationRotationScale3D"
                raise NotImplementedError("TranslationRotationScale3D")
            else:
                raise ValueError(f"Unknown affine transform representation: {transform.affine3d}")

            directed_affine3d = build_dense_union(
                affine3d_union_type.type, discriminant=discriminant_affine3d, child=affine3d
            )
            transform3d = build_dense_union(
                directed_affine3d_union_type.type, discriminant=discriminant_affine3d_direction, child=directed_affine3d
            )
        else:
            raise ValueError(f"Unknown transform type: {transform}")

        storage = build_dense_union(
            data_type=Transform3DType.storage_type, discriminant=discriminant_transform3d, child=transform3d
        )

        # TODO(clement) enable extension type wrapper
        # return cast(Transform3DArray, pa.ExtensionArray.from_storage(Transform3DType(), storage))
        return storage  # type: ignore[no-any-return]


Transform3DType = ComponentTypeFactory(
    "Transform3DType", Transform3DArray, REGISTERED_COMPONENT_NAMES["rerun.transform3d"]
)

pa.register_extension_type(Transform3DType())
