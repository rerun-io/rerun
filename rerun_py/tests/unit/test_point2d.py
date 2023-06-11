import itertools

import numpy as np
import rerun2 as rr
from rerun2 import components as rrc


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
        # Point2DArrayLike: npt.NDArray[np.float32]
        [1, 2, 3, 4],
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
        # RadiusArrayLike: Sequence[RadiusLike]: npt.NDArray[np.float32]
        np.array([42, 43]),
    ]

    # TODO: continue on that trend

    colors_arrays = [
        None,
        [
            rrc.Color(0xFF0000FF),
            rrc.Color(0x00FF00FF),
        ],
    ]

    labels_arrays = [
        None,
        "hey",
        ["hello", "friend", "o"],
        rrc.Label("sup"),
        [
            rrc.Label("yo"),
            rrc.Label("oi"),
        ],
    ]

    draw_orders_arrays = [
        None,
        300,
        rrc.DrawOrder(300),
    ]

    all_permuted_arrays = list(
        itertools.product(
            *[
                points_arrays,
                radii_arrays,
                colors_arrays,
                labels_arrays,
                draw_orders_arrays,
            ]
        )
    )

    for points, radii, colors, labels, draw_order in all_permuted_arrays:
        print(
            f"rr.Points2D(\n"
            f"    {points}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    draw_order={draw_order}\n"
            f")"
        )
        arch = rr.Points2D(points, radii=radii, colors=colors, labels=labels, draw_order=draw_order)

        assert arch.points == rrc.Point2DArray.from_similar([[1.0, 2.0], [3.0, 4.0]])
        assert arch.radii == rrc.RadiusArray.from_similar([42, 43] if radii is not None else [])
        print(colors_arrays)
        assert arch.colors == rrc.ColorArray.from_similar(
            [
                rrc.Color(0xFF0000FF),
                rrc.Color(0x00FF00FF),
            ]
            if colors is not None
            else []
        )
        print(arch)
        print()


if __name__ == "__main__":
    test_points2d()
