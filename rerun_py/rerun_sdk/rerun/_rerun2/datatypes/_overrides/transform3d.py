from __future__ import annotations

from typing import TYPE_CHECKING, cast

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from .. import (
        Mat3x3,
        Rotation3D,
        RotationAxisAngle,
        Scale3D,
        Transform3DArrayLike,
        TranslationAndMat3x3,
        TranslationRotationScale3D,
        Vec3D,
    )

# TODO(#2623): lots of boilerplate here that could be auto-generated
# To address that:
# 1) rewrite everything in the form of `xxx_native_to_pa_array()`
# 2) higher level `xxx_native_to_pa_array()` should call into lower level `xxx::from_similar()`
# 3) identify regularities and auto-gen them


def _union_discriminant_type(data_type: pa.DenseUnionType, discriminant: str) -> pa.DataType:
    """Return the data type of the given discriminant."""
    return next(f.type for f in list(data_type) if f.name == discriminant)


def _build_dense_union(data_type: pa.DenseUnionType, discriminant: str, child: pa.Array) -> pa.Array:
    """
    Build a dense UnionArray given the `data_type`, a discriminant, and the child value array.

    If the discriminant string doesn't match any possible value, a `ValueError` is raised.
    """
    try:
        idx = [f.name for f in list(data_type)].index(discriminant)
        type_ids = pa.array([idx] * len(child), type=pa.int8())
        value_offsets = pa.array(range(len(child)), type=pa.int32())

        children = [pa.nulls(0, type=f.type) for f in list(data_type)]
        try:
            children[idx] = child.cast(data_type[idx].type, safe=False)
        except pa.ArrowInvalid:
            # Since we're having issues with nullability in union types (see below),
            # the cast sometimes fails but can be skipped.
            children[idx] = child

        return pa.Array.from_buffers(
            type=data_type,
            length=len(child),
            buffers=[None, type_ids.buffers()[1], value_offsets.buffers()[1]],
            children=children,
        )

    except ValueError as e:
        raise ValueError(e.args)


def _build_struct_array_from_axis_angle_rotation(
    rotation: RotationAxisAngle, axis_angle_type: pa.StructType
) -> pa.StructArray:
    axis = pa.FixedSizeListArray.from_arrays(
        np.array(rotation.axis.xyz, dtype=np.float32).flatten(), type=axis_angle_type["axis"].type
    )

    angle = pa.array([rotation.angle.inner], type=pa.float32())
    if rotation.angle.kind == "degrees":
        angle_variant = "Degrees"
    else:
        angle_variant = "Radians"
    angle_pa_arr = _build_dense_union(axis_angle_type["angle"].type, angle_variant, angle)

    return pa.StructArray.from_arrays(
        [
            axis,
            angle_pa_arr,
        ],
        fields=list(axis_angle_type),
    )


def _build_union_array_from_scale(scale: Scale3D | None, type_: pa.DenseUnionType) -> pa.Array:
    from .. import Vec3D

    if scale is None:
        return pa.nulls(1, type=type_)

    scale_arm = scale.inner

    if np.isscalar(scale_arm):
        scale_discriminant = "Uniform"
        scale_pa_arr = pa.array([scale_arm], type=pa.float32())
    else:
        scale_discriminant = "ThreeD"
        scale_pa_arr = pa.FixedSizeListArray.from_arrays(
            cast(Vec3D, scale_arm).xyz, type=_union_discriminant_type(type_, scale_discriminant)
        )

    return _build_dense_union(type_, scale_discriminant, scale_pa_arr)


def _build_union_array_from_rotation(rotation: Rotation3D | None, type_: pa.DenseUnionType) -> pa.Array:
    from .. import Quaternion, RotationAxisAngle

    if rotation is None:
        return pa.nulls(1, type=type_)

    rotation_arm = rotation.inner

    if isinstance(rotation_arm, RotationAxisAngle):
        rotation_discriminant = "AxisAngle"
        axis_angle_type = _union_discriminant_type(type_, rotation_discriminant)
        stored_rotation = _build_struct_array_from_axis_angle_rotation(
            rotation_arm, cast(pa.StructType, axis_angle_type)
        )
    elif isinstance(rotation_arm, Quaternion):
        rotation_discriminant = "Quaternion"
        np_rotation = np.array(rotation_arm.xyzw, dtype=np.float32).flatten()
        stored_rotation = pa.FixedSizeListArray.from_arrays(
            np_rotation, type=_union_discriminant_type(type_, rotation_discriminant)
        )
    else:
        raise ValueError(
            f"Unknown 3d rotation representation: {rotation_arm} (expected `Rotation3D`, `RotationAxisAngle`, "
            "`Quaternion`, or `None`."
        )

    return _build_dense_union(type_, rotation_discriminant, stored_rotation)


def _optional_mat3x3_to_arrow(mat: Mat3x3 | None) -> pa.Array:
    from .. import Mat3x3Type

    if mat is None:
        return pa.nulls(1, type=Mat3x3Type().storage_type)
    else:
        return pa.FixedSizeListArray.from_arrays(mat.coeffs, type=Mat3x3Type().storage_type)


def _optional_translation_to_arrow(translation: Vec3D | None) -> pa.Array:
    from .. import Vec3DType

    if translation is None:
        return pa.nulls(1, type=Vec3DType().storage_type)
    else:
        return pa.FixedSizeListArray.from_arrays(translation.xyz, type=Vec3DType().storage_type)


def _build_struct_array_from_translation_mat3x3(
    translation_mat3: TranslationAndMat3x3, type_: pa.StructType
) -> pa.StructArray:
    translation = _optional_translation_to_arrow(translation_mat3.translation)
    matrix = _optional_mat3x3_to_arrow(translation_mat3.matrix)

    return pa.StructArray.from_arrays(
        [
            translation,
            matrix,
            pa.array([translation_mat3.from_parent], type=pa.bool_()),
        ],
        fields=list(type_),
    )


def _build_struct_array_from_translation_rotation_scale(
    transform: TranslationRotationScale3D, type_: pa.StructType
) -> pa.StructArray:
    translation = _optional_translation_to_arrow(transform.translation)
    rotation = _build_union_array_from_rotation(transform.rotation, type_["rotation"].type)
    scale = _build_union_array_from_scale(transform.scale, type_["scale"].type)

    return pa.StructArray.from_arrays(
        [
            translation,
            rotation,
            scale,
            pa.array([transform.from_parent], type=pa.bool_()),
        ],
        fields=list(type_),
    )


def transform3d_native_to_pa_array(data: Transform3DArrayLike, data_type: pa.DataType) -> pa.Array:
    from .. import Transform3D, TranslationAndMat3x3, TranslationRotationScale3D

    if isinstance(data, Transform3D):
        data = data.inner

    if isinstance(data, TranslationAndMat3x3):
        discriminant = "TranslationAndMat3x3"
        repr_type = _union_discriminant_type(data_type, discriminant)
        transform_repr = _build_struct_array_from_translation_mat3x3(data, cast(pa.StructType, repr_type))
    elif isinstance(data, TranslationRotationScale3D):
        discriminant = "TranslationRotationScale"
        repr_type = _union_discriminant_type(data_type, discriminant)
        transform_repr = _build_struct_array_from_translation_rotation_scale(data, cast(pa.StructType, repr_type))
    else:
        raise ValueError(
            f"unknown transform 3d value: {data} (expected `Transform3D`, `TranslationAndMat3x3`, or "
            "`TranslationRotationScale`"
        )

    storage = _build_dense_union(data_type, discriminant, transform_repr)

    # TODO(clement) enable extension type wrapper
    # return cast(Transform3DArray, pa.ExtensionArray.from_storage(Transform3DType(), storage))
    return storage  # type: ignore[no-any-return]
