from __future__ import annotations

import itertools
from typing import Optional, cast

import rerun as rr
from rerun.components import HalfSizes3DBatch, Position3DBatch, RadiusArrayLike, Rotation3DBatch
from rerun.datatypes import ClassIdArrayLike, Rgba32ArrayLike, Rotation3DArrayLike, Utf8ArrayLike
from rerun.datatypes.vec3d import Vec3DArrayLike

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    expected_rotations,
    labels_arrays,
    labels_expected,
    radii_arrays,
    radii_expected,
    rotations_arrays,
)
from .common_arrays import (
    vec3ds_arrays as centers_arrays,
)
from .common_arrays import (
    vec3ds_arrays as half_sizes_arrays,
)
from .common_arrays import (
    vec3ds_expected as centers_expected,
)
from .common_arrays import (
    vec3ds_expected as half_sizes_expected,
)


def test_boxes3d() -> None:
    all_arrays = itertools.zip_longest(
        half_sizes_arrays,
        centers_arrays,
        rotations_arrays,
        colors_arrays,
        radii_arrays,
        labels_arrays,
        class_ids_arrays,
    )

    for half_sizes, centers, rotations, colors, radii, labels, class_ids in all_arrays:
        half_sizes = half_sizes if half_sizes is not None else half_sizes_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        half_sizes = cast(Vec3DArrayLike, half_sizes)
        centers = cast(Vec3DArrayLike, centers)
        rotations = cast(Rotation3DArrayLike, rotations)
        radii = cast(Optional[RadiusArrayLike], radii)
        colors = cast(Optional[Rgba32ArrayLike], colors)
        labels = cast(Optional[Utf8ArrayLike], labels)
        class_ids = cast(Optional[ClassIdArrayLike], class_ids)

        print(
            f"rr.Boxes3D(\n"
            f"    half_sizes={half_sizes}\n"
            f"    rotations={rotations}\n"
            f"    centers={centers}\n"
            f"    radii={radii!r}\n"
            f"    colors={colors!r}\n"
            f"    labels={labels!r}\n"
            f"    class_ids={class_ids!r}\n"
            f")"
        )
        arch = rr.Boxes3D(
            half_sizes=half_sizes,
            centers=centers,
            rotations=rotations,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
        )
        print(f"{arch}\n")

        assert arch.half_sizes == half_sizes_expected(half_sizes, HalfSizes3DBatch)
        assert arch.centers == centers_expected(centers, Position3DBatch)
        assert arch.rotations == expected_rotations(rotations, Rotation3DBatch)
        assert arch.colors == colors_expected(colors)
        assert arch.radii == radii_expected(radii)
        assert arch.labels == labels_expected(labels)
        assert arch.class_ids == class_ids_expected(class_ids)


def test_with_sizes() -> None:
    assert rr.Boxes3D(sizes=[1, 2, 3]) == rr.Boxes3D(half_sizes=[0.5, 1, 1.5])


def test_with_centers_and_sizes() -> None:
    assert rr.Boxes3D(centers=[1, 2, 3], sizes=[4, 6, 8]) == rr.Boxes3D(centers=[1, 2, 3], half_sizes=[2, 3, 4])


def test_with_mins_and_sizes() -> None:
    assert rr.Boxes3D(mins=[-1, -1, -1], sizes=[2, 4, 2]) == rr.Boxes3D(centers=[0, 1, 0], half_sizes=[1, 2, 1])


if __name__ == "__main__":
    test_boxes3d()
    test_with_sizes()
    test_with_centers_and_sizes()
    test_with_mins_and_sizes()
