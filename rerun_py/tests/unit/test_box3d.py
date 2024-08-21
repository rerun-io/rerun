from __future__ import annotations

import itertools
from typing import Optional, cast

import rerun as rr
from rerun.components import HalfSize3DBatch, PoseRotationAxisAngleBatch, PoseRotationQuatBatch, PoseTranslation3DBatch
from rerun.datatypes import (
    ClassIdArrayLike,
    Float32ArrayLike,
    QuaternionArrayLike,
    Rgba32ArrayLike,
    RotationAxisAngleArrayLike,
    Utf8ArrayLike,
    Vec3DArrayLike,
)

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    expected_rotation_axis_angles,
    labels_arrays,
    labels_expected,
    quaternions_arrays,
    quaternions_expected,
    radii_arrays,
    radii_expected,
    rotation_axis_angle_arrays,
    vec3ds_arrays as centers_arrays,
    vec3ds_arrays as half_sizes_arrays,
    vec3ds_expected as centers_expected,
    vec3ds_expected as half_sizes_expected,
)


def test_boxes3d() -> None:
    fill_mode_arrays = [
        None,
        rr.components.FillMode.Solid,
        rr.components.FillMode.MajorWireframe,
        rr.components.FillMode.DenseWireframe,
    ]

    all_arrays = itertools.zip_longest(
        half_sizes_arrays,
        centers_arrays,
        rotation_axis_angle_arrays,
        quaternions_arrays,
        colors_arrays,
        radii_arrays,
        fill_mode_arrays,
        labels_arrays,
        class_ids_arrays,
    )

    for (
        half_sizes,
        centers,
        rotation_axis_angles,
        quaternions,
        colors,
        radii,
        fill_mode,
        labels,
        class_ids,
    ) in all_arrays:
        half_sizes = half_sizes if half_sizes is not None else half_sizes_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        half_sizes = cast(Vec3DArrayLike, half_sizes)
        centers = cast(Vec3DArrayLike, centers)
        rotation_axis_angles = cast(RotationAxisAngleArrayLike, rotation_axis_angles)
        quaternions = cast(QuaternionArrayLike, quaternions)
        radii = cast(Optional[Float32ArrayLike], radii)
        colors = cast(Optional[Rgba32ArrayLike], colors)
        labels = cast(Optional[Utf8ArrayLike], labels)
        class_ids = cast(Optional[ClassIdArrayLike], class_ids)
        fill_mode = cast(Optional[rr.components.FillMode], fill_mode)

        print(
            f"rr.Boxes3D(\n"
            f"    half_sizes={half_sizes}\n"
            f"    rotation_axis_angles={rotation_axis_angles}\n"
            f"    quaternions={quaternions}\n"
            f"    centers={centers}\n"
            f"    radii={radii!r}\n"
            f"    colors={colors!r}\n"
            f"    labels={labels!r}\n"
            f"    fill_mode={fill_mode!r}\n"
            f"    class_ids={class_ids!r}\n"
            f")"
        )
        arch = rr.Boxes3D(
            half_sizes=half_sizes,
            centers=centers,
            rotation_axis_angles=rotation_axis_angles,
            quaternions=quaternions,
            radii=radii,
            colors=colors,
            labels=labels,
            fill_mode=fill_mode,
            class_ids=class_ids,
        )
        print(f"{arch}\n")

        assert arch.half_sizes == half_sizes_expected(half_sizes, HalfSize3DBatch)
        assert arch.centers == centers_expected(centers, PoseTranslation3DBatch)
        assert arch.rotation_axis_angles == expected_rotation_axis_angles(
            rotation_axis_angles, PoseRotationAxisAngleBatch
        )
        assert arch.quaternions == quaternions_expected(quaternions, PoseRotationQuatBatch)
        assert arch.colors == colors_expected(colors)
        assert arch.radii == radii_expected(radii)
        assert arch.fill_mode == rr.components.FillModeBatch._optional(fill_mode)
        assert arch.labels == labels_expected(labels)
        assert arch.class_ids == class_ids_expected(class_ids)


# Test `rotations` parameter
def test_boxes3d_rotations_quat() -> None:
    all_arrays = itertools.zip_longest(
        half_sizes_arrays,
        quaternions_arrays,
    )

    for (
        half_sizes,
        quaternions,
    ) in all_arrays:
        half_sizes = half_sizes if half_sizes is not None else half_sizes_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        half_sizes = cast(Vec3DArrayLike, half_sizes)
        quaternions = cast(QuaternionArrayLike, quaternions)

        print(f"rr.Boxes3D(\n" f"    half_sizes={half_sizes}\n" f"    rotations={quaternions!r}\n" f")")
        arch = rr.Boxes3D(
            half_sizes=half_sizes,
            rotations=quaternions,
        )
        print(f"{arch.quaternions}\n")
        if arch.quaternions is not None:
            print(f"{arch.quaternions.as_arrow_array()}\n")

        assert arch.half_sizes == half_sizes_expected(half_sizes, HalfSize3DBatch)
        assert arch.quaternions == quaternions_expected(quaternions, PoseRotationQuatBatch)


# Test `rotations` parameter
def test_boxes3d_rotations_rotation_axis_angle() -> None:
    all_arrays = itertools.zip_longest(
        half_sizes_arrays,
        rotation_axis_angle_arrays,
    )

    for (
        half_sizes,
        rotation_axis_angles,
    ) in all_arrays:
        half_sizes = half_sizes if half_sizes is not None else half_sizes_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        half_sizes = cast(Vec3DArrayLike, half_sizes)
        rotation_axis_angles = cast(RotationAxisAngleArrayLike, rotation_axis_angles)

        print(f"rr.Boxes3D(\n" f"    half_sizes={half_sizes}\n" f"    rotations={rotation_axis_angles}\n" f")")
        arch = rr.Boxes3D(
            half_sizes=half_sizes,
            rotations=rotation_axis_angles,
        )
        print(f"{arch.quaternions}\n")
        if arch.quaternions is not None:
            print(f"{arch.quaternions.as_arrow_array()}\n")

        if arch.rotation_axis_angles is not None:
            print(f"{arch.rotation_axis_angles.as_arrow_array()}\n")

        assert arch.half_sizes == half_sizes_expected(half_sizes, HalfSize3DBatch)

        print(
            f"{arch.rotation_axis_angles} == {expected_rotation_axis_angles(rotation_axis_angles, PoseRotationAxisAngleBatch)}"
        )

        assert arch.rotation_axis_angles == expected_rotation_axis_angles(
            rotation_axis_angles, PoseRotationAxisAngleBatch
        )


def test_with_sizes() -> None:
    assert rr.Boxes3D(sizes=[1, 2, 3]) == rr.Boxes3D(half_sizes=[0.5, 1, 1.5])


def test_with_centers_and_sizes() -> None:
    assert rr.Boxes3D(centers=[1, 2, 3], sizes=[4, 6, 8]) == rr.Boxes3D(centers=[1, 2, 3], half_sizes=[2, 3, 4])


def test_with_mins_and_sizes() -> None:
    assert rr.Boxes3D(mins=[-1, -1, -1], sizes=[2, 4, 2]) == rr.Boxes3D(centers=[0, 1, 0], half_sizes=[1, 2, 1])
