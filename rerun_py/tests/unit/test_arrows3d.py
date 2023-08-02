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
)


def test_arrows3d() -> None:
    arrows_arrays: list[rrd.Arrow3DArrayLike] = [
        [],
        # Arrow3DArrayLike: Sequence[Arrow3DLike]: Arrow3D
        [
            rrd.Arrow3D(origin=[1, 2, 3], vector=[4, 5, 6]),
            rrd.Arrow3D(origin=[10, 20, 30], vector=[40, 50, 60]),
        ],
    ]

    all_arrays = itertools.zip_longest(
        arrows_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        class_ids_arrays,
        instance_keys_arrays,
    )

    for arrows, radii, colors, labels, class_ids, instance_keys in all_arrays:
        arrows = arrows if arrows is not None else arrows_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        arrows = cast(Optional[rrd.Arrow3DArrayLike], arrows)
        radii = cast(Optional[rrc.RadiusArrayLike], radii)
        colors = cast(Optional[rrc.ColorArrayLike], colors)
        labels = cast(Optional[rrc.LabelArrayLike], labels)
        class_ids = cast(Optional[rrc.ClassIdArrayLike], class_ids)
        instance_keys = cast(Optional[rrc.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr2.Arrows3D(\n"
            f"    {arrows}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    class_ids={class_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.Arrows3D(
            arrows,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.arrows == rrc.Arrow3DArray.from_similar(
            []
            if is_empty(arrows)
            else [
                rrd.Arrow3D(origin=[1, 2, 3], vector=[4, 5, 6]),
                rrd.Arrow3D(origin=[10, 20, 30], vector=[40, 50, 60]),
            ]
        )
        assert arch.radii == radii_expected(is_empty(radii))
        assert arch.colors == colors_expected(is_empty(colors))
        assert arch.labels == labels_expected(is_empty(labels))
        assert arch.class_ids == class_ids_expected(is_empty(class_ids))
        assert arch.instance_keys == instance_keys_expected(is_empty(instance_keys))


if __name__ == "__main__":
    test_arrows3d()
