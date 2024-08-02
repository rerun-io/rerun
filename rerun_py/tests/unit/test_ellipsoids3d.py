from __future__ import annotations

import itertools
from typing import Optional, cast

import rerun as rr
from rerun.components import HalfSize3DBatch, LeafRotationAxisAngleBatch, LeafRotationQuatBatch, LeafTranslation3DBatch
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
    expected_quaternions,
    expected_rotation_axis_angles,
    labels_arrays,
    labels_expected,
    quaternion_arrays,
    radii_arrays,
    radii_expected,
    rotation_axis_angle_arrays,
    vec3ds_arrays as centers_arrays,
    vec3ds_arrays as half_sizes_arrays,
    vec3ds_expected as centers_expected,
    vec3ds_expected as half_sizes_expected,
)


def test_ellipsoids() -> None:
    fill_mode_arrays = [None, rr.components.FillMode.Solid, rr.components.FillMode.Wireframe]

    all_arrays = itertools.zip_longest(
        half_sizes_arrays,
        centers_arrays,
        rotation_axis_angle_arrays,
        quaternion_arrays,
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
        line_radii,
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
        line_radii = cast(Optional[Float32ArrayLike], line_radii)
        colors = cast(Optional[Rgba32ArrayLike], colors)
        labels = cast(Optional[Utf8ArrayLike], labels)
        class_ids = cast(Optional[ClassIdArrayLike], class_ids)
        fill_mode = cast(Optional[rr.components.FillMode], fill_mode)

        print(
            f"rr.Ellipsoids3D(\n"
            f"    half_sizes={half_sizes}\n"
            f"    rotation_axis_angles={rotation_axis_angles}\n"
            f"    quaternions={quaternions}\n"
            f"    centers={centers}\n"
            f"    line_radii={line_radii!r}\n"
            f"    colors={colors!r}\n"
            f"    labels={labels!r}\n"
            f"    fill_mode={fill_mode!r}\n"
            f"    class_ids={class_ids!r}\n"
            f")"
        )
        arch = rr.Ellipsoids3D(
            half_sizes=half_sizes,
            centers=centers,
            rotation_axis_angles=rotation_axis_angles,
            quaternions=quaternions,
            line_radii=line_radii,
            colors=colors,
            labels=labels,
            fill_mode=fill_mode,
            class_ids=class_ids,
        )
        print(f"{arch}\n")

        assert arch.half_sizes == half_sizes_expected(half_sizes, HalfSize3DBatch)
        assert arch.centers == centers_expected(centers, LeafTranslation3DBatch)
        assert arch.rotation_axis_angles == expected_rotation_axis_angles(
            rotation_axis_angles, LeafRotationAxisAngleBatch
        )
        assert arch.quaternions == expected_quaternions(quaternions, LeafRotationQuatBatch)
        assert arch.colors == colors_expected(colors)
        assert arch.line_radii == radii_expected(line_radii)
        assert arch.fill_mode == rr.components.FillModeBatch._optional(fill_mode)
        assert arch.labels == labels_expected(labels)
        assert arch.class_ids == class_ids_expected(class_ids)
