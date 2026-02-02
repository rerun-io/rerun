from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, cast

import numpy as np
import rerun as rr
from rerun.components import Position3DBatch, Vector3DBatch

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

if TYPE_CHECKING:
    from rerun.datatypes import (
        ClassIdArrayLike,
        Float32ArrayLike,
        Float64ArrayLike,
        Rgba32ArrayLike,
        Utf8ArrayLike,
        Vec3DArrayLike,
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
        vectors = cast("Vec3DArrayLike", vectors)
        origins = cast("Vec3DArrayLike | None", origins)
        radii = cast("Float32ArrayLike | None", radii)
        colors = cast("Rgba32ArrayLike | None", colors)
        labels = cast("Utf8ArrayLike | None", labels)
        class_ids = cast("ClassIdArrayLike | None", class_ids)

        print(
            f"E: rr.Arrows3D(\n"
            f"    vectors={vectors}\n"
            f"    origins={origins!r}\n"
            f"    radii={radii!r}\n"
            f"    colors={colors!r}\n"
            f"    labels={labels!r}\n"
            f"    class_ids={class_ids!r}\n"
            f")",
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


CASES: list[tuple[Float64ArrayLike, Float64ArrayLike]] = [
    (
        [],
        [],
    ),
    (
        [[1.0, 1.0, 1.0]],
        [[[1.0, 1.0, 1.0]]],
    ),
    ([[[1.0, 1.0, 1.0]] for _ in range(3)], [[[1.0, 1.0, 1.0]] for _ in range(3)]),
    ([[2.1, 3.2, 4.3] for _ in range(333)], [[[2.1, 3.2, 4.3]] for _ in range(333)]),
    (np.ones((30, 3)), np.ones((30, 1, 3)).tolist()),
    (np.ones((3, 1, 3)), np.ones((3, 1, 3)).tolist()),
    (np.ones((1, 3)), np.ones((1, 1, 3)).tolist()),
]


def test_arrows3d_columnar() -> None:
    for input, expected in CASES:
        data = [*rr.Arrows3D.columns(vectors=input, origins=np.zeros_like(input))]
        assert np.allclose(np.asarray(data[0].as_arrow_array().to_pylist()), np.asarray(expected))


if __name__ == "__main__":
    test_arrows3d()
