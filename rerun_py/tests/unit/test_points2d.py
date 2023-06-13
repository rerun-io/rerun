from __future__ import annotations

import itertools

import numpy as np
# import rerun2 as rr
# from rerun2 import components as rrc
from rerun_sdk import rerun2 as rr
from rerun_sdk.rerun2 import components as rrc

# TODO(cmc): roundtrips (serialize in python, deserialize in rust)


def test_points2d() -> None:
    points_arrays = [
        # Point2DArrayLike: Sequence[Point2DLike]: Point2D
        [
            rrc.Point2D([1, 2]),
            rrc.Point2D([3, 4]),
        ],
        # Point2DArrayLike: Sequence[Point2DLike]: npt.NDArray[np.float32]
        [
            np.array([1, 2]),
            np.array([3, 4]),
        ],
        # Point2DArrayLike: Sequence[Point2DLike]: Tuple[float, float]
        [[1, 2], [3, 4]],
        # Point2DArrayLike: Sequence[Point2DLike]: Sequence[float]
        [1, 2, 3, 4],
        # Point2DArrayLike: npt.NDArray[np.float32]
        np.array([[1, 2], [3, 4]]),
        # Point2DArrayLike: Sequence[Point2DLike]
        [
            rrc.Point2D([1, 2]),
            rrc.Point2D([3, 4]),
        ],
        # Point2DArrayLike: npt.NDArray[np.float32]
        np.array([1, 2, 3, 4]),
    ]

    radii_arrays = [
        None,
        # RadiusArrayLike: Sequence[RadiusLike]: float
        [42, 43],
        # RadiusArrayLike: Sequence[RadiusLike]: Radius
        [
            rrc.Radius(42),
            rrc.Radius(43),
        ],
        # RadiusArrayLike: npt.NDArray[np.float32]
        np.array([42, 43]),
    ]

    # TODO: color

    labels_arrays = [
        None,
        # LabelArrayLike: Sequence[LabelLike]: str
        ["hello", "friend"],
        # LabelArrayLike: Sequence[LabelLike]: Label
        [
            rrc.Label("hello"),
            rrc.Label("friend"),
        ],
    ]

    all_permuted_arrays = list(
        itertools.product( # type: ignore[call-overload]
            *[
                points_arrays,
                radii_arrays,
                labels_arrays,
            ]
        )
    )

    for points, radii, labels in all_permuted_arrays:
        print(
            f"rr.Points2D(\n"
            f"    {points}\n"
            f"    radii={radii}\n"
            f"    labels={labels}\n"
            f")"
        )
        arch = rr.Points2D(points, radii=radii, labels=labels)
        print(f"{arch}\n")

        assert arch.points == rrc.Point2DArray.from_similar([[1.0, 2.0], [3.0, 4.0]])
        assert arch.radii == rrc.RadiusArray.from_similar([42, 43] if radii is not None else [])
        assert arch.labels == rrc.LabelArray.from_similar(["hello", "friend"] if labels is not None else [])


if __name__ == "__main__":
    test_points2d()
