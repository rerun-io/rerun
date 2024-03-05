from __future__ import annotations

import itertools
from typing import Optional, cast

import rerun as rr
from rerun.components import Position3DBatch, RadiusArrayLike, Vector3DBatch
from rerun.datatypes import ClassIdArrayLike, Rgba32ArrayLike, Utf8ArrayLike, Vec3DArrayLike

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
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
    )

    for vectors, origins, radii, colors, labels, class_ids in all_arrays:
        vectors = vectors if vectors is not None else vectors_arrays[-1]
        origins = origins if origins is not None else origins_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        vectors = cast(Vec3DArrayLike, vectors)
        origins = cast(Optional[Vec3DArrayLike], origins)
        radii = cast(Optional[RadiusArrayLike], radii)
        colors = cast(Optional[Rgba32ArrayLike], colors)
        labels = cast(Optional[Utf8ArrayLike], labels)
        class_ids = cast(Optional[ClassIdArrayLike], class_ids)

        print(
            f"E: rr.Arrows3D(\n"
            f"    vectors={vectors}\n"
            f"    origins={origins!r}\n"
            f"    radii={radii!r}\n"
            f"    colors={colors!r}\n"
            f"    labels={labels!r}\n"
            f"    class_ids={class_ids!r}\n"
            f")"
        )
        arch = rr.Arrows3D(
            vectors=vectors,
            origins=origins,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
        )
        print(f"A: {arch}\n")

        assert arch.vectors == vec3ds_expected(vectors, Vector3DBatch)
        assert arch.origins == vec3ds_expected(origins, Position3DBatch)
        assert arch.radii == radii_expected(radii)
        assert arch.colors == colors_expected(colors)
        assert arch.labels == labels_expected(labels)
        assert arch.class_ids == class_ids_expected(class_ids)


if __name__ == "__main__":
    test_arrows3d()
