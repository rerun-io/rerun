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
    instance_keys_arrays,
    instance_keys_expected,
    is_empty,
    labels_arrays,
    labels_expected,
    radii_arrays,
    radii_expected,
    vec3ds_arrays,
    vec3ds_expected,
)


def test_arrows3d() -> None:
    vectors_arrays = vec3ds_arrays
    origins_arrays = vec3ds_arrays

    all_arrays = itertools.zip_longest(
        vectors_arrays,
        origins_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        class_ids_arrays,
        instance_keys_arrays,
    )

    for vectors, origins, radii, colors, labels, class_ids, instance_keys in all_arrays:
        vectors = vectors if vectors is not None else vectors_arrays[-1]
        origins = origins if origins is not None else origins_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        vectors = cast(rrd.Vec3DArrayLike, vectors)
        origins = cast(Optional[rrd.Vec3DArrayLike], origins)
        radii = cast(Optional[rrc.RadiusArrayLike], radii)
        colors = cast(Optional[rrc.ColorArrayLike], colors)
        labels = cast(Optional[rrc.LabelArrayLike], labels)
        class_ids = cast(Optional[rrc.ClassIdArrayLike], class_ids)
        instance_keys = cast(Optional[rrc.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr2.Arrows3D(\n"
            f"    {vectors}\n"
            f"    origins={origins}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    class_ids={class_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.Arrows3D(
            vectors,
            origins=origins,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.vectors == vec3ds_expected(is_empty(vectors), rrc.Vector3DArray)
        assert arch.origins == vec3ds_expected(is_empty(origins), rrc.Origin3DArray)
        assert arch.radii == radii_expected(is_empty(radii))
        assert arch.colors == colors_expected(is_empty(colors))
        assert arch.labels == labels_expected(is_empty(labels))
        assert arch.class_ids == class_ids_expected(is_empty(class_ids))
        assert arch.instance_keys == instance_keys_expected(is_empty(instance_keys))


if __name__ == "__main__":
    test_arrows3d()
