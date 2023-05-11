from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Sequence

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.components import (
    REGISTERED_COMPONENT_NAMES,
    ComponentTypeFactory,
    build_dense_union,
    union_discriminant_type,
)

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

    translation: npt.ArrayLike | None = None
    """3D translation vector, applied after the matrix. Uses (0, 0, 0) if not set."""

    matrix: npt.ArrayLike | None = None
    """The column-major 3x3 matrix for scale, rotation & skew matrix. Uses identity if not set."""


@dataclass
class TranslationRotationScale3D:
    """Representation of an affine transform via separate translation, rotation & scale."""

    translation: npt.ArrayLike | None = None
    """3D translation vector, applied last."""

    rotation: npt.ArrayLike | RotationAxisAngle | None = None
    """3D rotation, represented as a (xyzw) quaternion or axis + angle, applied second."""

    scale: npt.ArrayLike | float | None = None
    """3D scaling either a 3D vector, scalar or None. Applied first."""


@dataclass
class RotationAxisAngle:
    """3D rotation expressed via a rotation axis and angle."""

    axis: npt.ArrayLike
    """3D rotation axis."""

    degrees: float | None = None
    """3D rotation angle in degrees. Only one of `degrees` or `radians` should be set."""

    radians: float | None = None
    """3D rotation angle in radians. Only one of `degrees` or `radians` should be set."""


def translation_to_fixed_size_array(
    translation: npt.ArrayLike | None, struct_type: pa.StructType
) -> pa.FixedSizeListArray:
    translation = (0, 0, 0) if translation is None else translation
    return pa.FixedSizeListArray.from_arrays(
        np.array(translation, dtype=np.float32).flatten(), type=struct_type["translation"].type
    )


def build_struct_array_from_translation_mat3(
    translation_mat3: TranslationMatrix3x3, type: pa.StructType
) -> pa.StructArray:
    translation = translation_to_fixed_size_array(translation_mat3.translation, type)

    matrix = np.eye(3) if translation_mat3.matrix is None else translation_mat3.matrix
    matrix = np.array(matrix, dtype=np.float32).flatten()

    return pa.StructArray.from_arrays(
        [
            translation,
            pa.FixedSizeListArray.from_arrays(matrix, type=type["matrix"].type),
        ],
        fields=list(type),
    )


def build_struct_array_from_axis_angle_rotation(
    rotation: RotationAxisAngle, axis_angle_type: pa.StructType
) -> pa.StructArray:
    if rotation.degrees is None and rotation.radians is None:
        raise ValueError("RotationAxisAngle must have either degrees or radians set")
    if rotation.degrees is not None and rotation.radians is not None:
        raise ValueError("RotationAxisAngle must have either degrees or radians set, not both")

    axis = np.array(rotation.axis, dtype=np.float32).flatten()
    axis = pa.FixedSizeListArray.from_arrays(axis, type=axis_angle_type["axis"].type)

    if rotation.degrees is not None:
        angle = pa.array([rotation.degrees], type=pa.float32())
        angle_variant = "Degrees"
    else:
        angle = pa.array([rotation.radians], type=pa.float32())
        angle_variant = "Radians"
    angle = build_dense_union(axis_angle_type["angle"].type, angle_variant, angle)

    return pa.StructArray.from_arrays(
        [
            axis,
            angle,
        ],
        fields=list(axis_angle_type),
    )


def build_union_array_from_rotation(rotation: npt.ArrayLike | RotationAxisAngle | None, type: pa.DenseUnionType):
    if rotation is None:
        rotation_discriminant = "Identity"
        rotation = pa.array([False])
    elif isinstance(rotation, RotationAxisAngle):
        rotation_discriminant = "AxisAngle"
        axis_angle_type = union_discriminant_type(type, rotation_discriminant)
        rotation = build_struct_array_from_axis_angle_rotation(rotation, axis_angle_type)
    else:
        rotation_discriminant = "Quaternion"
        rotation = np.array(rotation, dtype=np.float32).flatten()
        if len(rotation) != 4:
            raise ValueError(f"Quaternion must have 4 elements, got {len(rotation)}")
        rotation = pa.FixedSizeListArray.from_arrays(
            rotation, type=union_discriminant_type(type, rotation_discriminant)
        )

    return build_dense_union(type, rotation_discriminant, rotation)


def build_union_array_from_scale(scale: npt.ArrayLike | float | None, type: pa.DenseUnionType):
    if scale is None:
        scale_discriminant = "Unit"
        scale = pa.array([False])
    elif np.isscalar(scale):
        scale_discriminant = "Uniform"
        scale = pa.array([scale], type=pa.float32())
    else:
        scale_discriminant = "ThreeD"
        scale = np.array(scale, dtype=np.float32).flatten()
        if len(scale) != 3:
            raise ValueError(f"Scale vector must have 3 elements, got {len(scale)}")
        scale = pa.FixedSizeListArray.from_arrays(scale, type=union_discriminant_type(type, scale_discriminant))

    return build_dense_union(type, scale_discriminant, scale)


def build_struct_array_from_translation_rotation_scale(
    transform: TranslationRotationScale3D, type: pa.StructType
) -> pa.StructArray:
    translation = translation_to_fixed_size_array(transform.translation, type)

    rotation = build_union_array_from_rotation(transform.rotation, type["rotation"].type)
    scale = build_union_array_from_scale(transform.scale, type["scale"].type)

    return pa.StructArray.from_arrays(
        [
            translation,
            rotation,
            scale,
        ],
        fields=list(type),
    )


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

            directed_affine3d_union_type = union_discriminant_type(
                Transform3DType.storage_type, discriminant_transform3d
            )
            affine3d_union_type = directed_affine3d_union_type[0].type  # both [0] and [1] are the same type

            if isinstance(transform.affine3d, TranslationMatrix3x3):
                discriminant_affine3d = "TranslationMatrix3x3"
                repr_type = union_discriminant_type(affine3d_union_type, discriminant_affine3d)
                affine3d = build_struct_array_from_translation_mat3(transform.affine3d, repr_type)
            elif isinstance(transform.affine3d, TranslationRotationScale3D):
                discriminant_affine3d = "TranslationRotationScale"
                repr_type = union_discriminant_type(affine3d_union_type, discriminant_affine3d)
                affine3d = build_struct_array_from_translation_rotation_scale(transform.affine3d, repr_type)
            else:
                raise ValueError(f"Unknown affine transform representation: {transform.affine3d}")

            directed_affine3d = build_dense_union(
                affine3d_union_type, discriminant=discriminant_affine3d, child=affine3d
            )
            transform3d = build_dense_union(
                directed_affine3d_union_type, discriminant=discriminant_affine3d_direction, child=directed_affine3d
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
