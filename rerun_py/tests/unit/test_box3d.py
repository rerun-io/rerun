from __future__ import annotations

import itertools
from typing import Optional, cast

import rerun.experimental as rr2
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    expected_rotations,
    instance_keys_arrays,
    instance_keys_expected,
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
        instance_keys_arrays,
    )

    for half_sizes, centers, rotations, colors, radii, labels, class_ids, instance_keys in all_arrays:
        half_sizes = half_sizes if half_sizes is not None else half_sizes_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        half_sizes = cast(rrd.Vec3DArrayLike, half_sizes)
        centers = cast(rrd.Vec3DArrayLike, centers)
        rotations = cast(rrd.Rotation3DArrayLike, rotations)
        radii = cast(Optional[rrc.RadiusArrayLike], radii)
        colors = cast(Optional[rrd.ColorArrayLike], colors)
        labels = cast(Optional[rrd.Utf8ArrayLike], labels)
        class_ids = cast(Optional[rrd.ClassIdArrayLike], class_ids)
        instance_keys = cast(Optional[rrc.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr2.Boxes3D(\n"
            f"    half_sizes={half_sizes}\n"
            f"    centers={centers}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    class_ids={class_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.Boxes3D(
            half_sizes=half_sizes,
            centers=centers,
            rotations=rotations,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.half_sizes == half_sizes_expected(half_sizes, rrc.HalfSizes3DArray)
        assert arch.centers == centers_expected(centers, rrc.Position3DArray)
        assert arch.rotations == expected_rotations(rotations, rrc.Rotation3DArray)
        assert arch.colors == colors_expected(colors)
        assert arch.radii == radii_expected(radii)
        assert arch.labels == labels_expected(labels)
        assert arch.class_ids == class_ids_expected(class_ids)
        assert arch.instance_keys == instance_keys_expected(instance_keys)


if __name__ == "__main__":
    test_boxes3d()
