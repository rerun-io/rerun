from __future__ import annotations

import itertools

import numpy as np
# import rerun2 as rr
# from rerun2 import components as rrc
from rerun_sdk import rerun2 as rr
from rerun_sdk.rerun2 import components as rrc


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

    all_permuted_arrays = list(
        itertools.product( # type: ignore[call-overload]
            *[
                points_arrays,
                radii_arrays,
            ]
        )
    )

    for points, radii in all_permuted_arrays:
        print(
            f"rr.Points2D(\n"
            f"    {points}\n"
            f"    radii={radii}\n"
            f")"
        )
        arch = rr.Points2D(points, radii=radii)

        assert arch.points == rrc.Point2DArray.from_similar([[1.0, 2.0], [3.0, 4.0]])
        assert arch.radii == rrc.RadiusArray.from_similar([42, 43] if radii is not None else [])
        print(f"{arch}\n")


if __name__ == "__main__":
    test_points2d()
