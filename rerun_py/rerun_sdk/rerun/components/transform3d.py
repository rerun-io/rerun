from __future__ import annotations

from dataclasses import dataclass

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.components import (
    REGISTERED_COMPONENT_NAMES,
    ComponentTypeFactory,
    build_dense_union,
    union_discriminant_type,
)
from rerun.components.quaternion import Quaternion
from rerun.components.vec import Vec3DType
from rerun.log import _normalize_matrix3

__all__ = [
    "Quaternion",
    "Rigid3D",
    "RotationAxisAngle",
    "Scale3D",
    "Transform3D",
    "Transform3DArray",
    "Transform3DType",
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


def optional_translation_to_arrow(translation: npt.ArrayLike | Translation3D | None) -> pa.UnionArray:
    # "unpack" rr.Translation3D first.
    if isinstance(translation, Translation3D):
        translation = translation.translation

    if translation is None:
        return pa.nulls(1, type=Vec3DType.storage_type)

    np_translation = np.array(translation, dtype=np.float32).flatten()
    if np_translation.size != 3:
        raise ValueError(f"Expected three dimensional translation vector, shape was instead {np_translation.shape}")
    return pa.FixedSizeListArray.from_arrays(np_translation, type=Vec3DType.storage_type)


def build_struct_array_from_translation_mat3(
    translation_mat3: TranslationAndMat3, type: pa.StructType
) -> pa.StructArray:
    translation = optional_translation_to_arrow(translation_mat3.translation)
    matrix = pa.FixedSizeListArray.from_arrays(_normalize_matrix3(translation_mat3.matrix), type=type["matrix"].type)

    return pa.StructArray.from_arrays(
        [
            translation,
            matrix,
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


def build_union_array_from_rotation(
    rotation: Quaternion | RotationAxisAngle | None, type: pa.DenseUnionType
) -> pa.UnionArray:
    if rotation is None:
        return pa.nulls(1, type=type)
    elif isinstance(rotation, RotationAxisAngle):
        rotation_discriminant = "AxisAngle"
        axis_angle_type = union_discriminant_type(type, rotation_discriminant)
        stored_rotation = build_struct_array_from_axis_angle_rotation(rotation, axis_angle_type)
    elif isinstance(rotation, Quaternion):
        rotation_discriminant = "Quaternion"
        np_rotation = np.array(rotation.xyzw, dtype=np.float32).flatten()
        stored_rotation = pa.FixedSizeListArray.from_arrays(
            np_rotation, type=union_discriminant_type(type, rotation_discriminant)
        )
    else:
        raise ValueError(
            f"Unknown 3d rotation representation: {rotation}. " + "Expected `RotationAxisAngle`/`Quaternion` or `None`."
        )

    return build_dense_union(type, rotation_discriminant, stored_rotation)


def build_union_array_from_scale(
    scale: Scale3D | npt.ArrayLike | float | None, type: pa.DenseUnionType
) -> pa.UnionArray:
    # "unpack" rr.Scale3D first.
    if isinstance(scale, Scale3D):
        scale = scale.scale

    if scale is None:
        return pa.nulls(1, type=type)
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
    translation = optional_translation_to_arrow(transform.translation)
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
    def from_transform(transform: Transform3D) -> Transform3DArray:
        """Build a `Transform3DArray` from a single transform."""

        transform_repr_union_type = Transform3DType.storage_type[0].type

        if isinstance(transform.transform, TranslationAndMat3):
            discriminant_affine3d = "TranslationAndMat3"
            repr_type = union_discriminant_type(transform_repr_union_type, discriminant_affine3d)
            transform_repr = build_struct_array_from_translation_mat3(transform.transform, repr_type)
        elif isinstance(transform.transform, TranslationRotationScale3D):
            discriminant_affine3d = "TranslationRotationScale"
            repr_type = union_discriminant_type(transform_repr_union_type, discriminant_affine3d)
            transform_repr = build_struct_array_from_translation_rotation_scale(transform.transform, repr_type)
        else:
            raise ValueError(
                f"Unknown transform 3d representation: {transform.transform} "
                + " Expected `TranslationAndMat3` or `TranslationRotationScale3D`."
            )

        storage = pa.StructArray.from_arrays(
            [
                build_dense_union(transform_repr_union_type, discriminant_affine3d, transform_repr),
                pa.array([transform.from_parent], type=Transform3DType.storage_type[1].type),
            ],
            fields=list(Transform3DType.storage_type),
        )

        # TODO(clement) enable extension type wrapper
        # return cast(Transform3DArray, pa.ExtensionArray.from_storage(Transform3DType(), storage))
        return storage  # type: ignore[no-any-return]


Transform3DType = ComponentTypeFactory(
    "Transform3DType", Transform3DArray, REGISTERED_COMPONENT_NAMES["rerun.transform3d"]
)

pa.register_extension_type(Transform3DType())
