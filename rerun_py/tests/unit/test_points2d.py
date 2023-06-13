from __future__ import annotations

import itertools

import numpy as np

# import rerun2 as rr
# from rerun2 import components as rrc
# NOTE: uncomment these to get a better auto-completion experience...
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
            np.array([1, 2], dtype=np.float32),
            np.array([3, 4], dtype=np.float32),
        ],
        # Point2DArrayLike: Sequence[Point2DLike]: Tuple[float, float]
        [[1, 2], [3, 4]],
        # Point2DArrayLike: Sequence[Point2DLike]: Sequence[float]
        [1, 2, 3, 4],
        # Point2DArrayLike: npt.NDArray[np.float32]
        np.array([[1, 2], [3, 4]], dtype=np.float32),
        # Point2DArrayLike: Sequence[Point2DLike]
        [
            rrc.Point2D([1, 2]),
            rrc.Point2D([3, 4]),
        ],
        # Point2DArrayLike: npt.NDArray[np.float32]
        np.array([1, 2, 3, 4], dtype=np.float32),
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
        np.array([42, 43], dtype=np.float32),
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

    draw_orders = [
        None,
        # DrawOrderLike: float
        300,
        # DrawOrderLike: DrawOrder
        rrc.DrawOrder(300),
    ]

    class_id_arrays = [
        # ClassIdArrayLike: Sequence[ClassIdLike]: int
        [126, 127],
        # ClassIdArrayLike: Sequence[ClassIdLike]: ClassId
        [rrc.ClassId(126), rrc.ClassId(127)],
        # ClassIdArrayLike: np.NDArray[np.uint8]
        np.array([126, 127], dtype=np.uint8),
        # ClassIdArrayLike: np.NDArray[np.uint16]
        np.array([126, 127], dtype=np.uint16),
        # ClassIdArrayLike: np.NDArray[np.uint32]
        np.array([126, 127], dtype=np.uint32),
        # ClassIdArrayLike: np.NDArray[np.uint64]
        np.array([126, 127], dtype=np.uint64),
    ]

    keypoint_id_arrays = [
        # KeypointIdArrayLike: Sequence[KeypointIdLike]: int
        [2, 3],
        # KeypointIdArrayLike: Sequence[KeypointIdLike]: KeypointId
        [rrc.KeypointId(2), rrc.KeypointId(3)],
        # KeypointIdArrayLike: np.NDArray[np.uint8]
        np.array([2, 3], dtype=np.uint8),
        # KeypointIdArrayLike: np.NDArray[np.uint16]
        np.array([2, 3], dtype=np.uint16),
        # KeypointIdArrayLike: np.NDArray[np.uint32]
        np.array([2, 3], dtype=np.uint32),
        # KeypointIdArrayLike: np.NDArray[np.uint64]
        np.array([2, 3], dtype=np.uint64),
    ]

    all_permuted_arrays = list(
        itertools.product(  # type: ignore[call-overload]
            *[
                points_arrays,
                radii_arrays,
                labels_arrays,
                draw_orders,
                class_id_arrays,
                keypoint_id_arrays,
            ]
        )
    )

    for points, radii, labels, draw_order, class_ids, keypoint_ids in all_permuted_arrays:
        print(
            f"rr.Points2D(\n"
            f"    {points}\n"
            f"    radii={radii}\n"
            f"    labels={labels}\n"
            f"    draw_order={draw_order}\n"
            f"    class_ids={class_ids}\n"
            f"    keypoint_ids={keypoint_ids}\n"
            f")"
        )
        arch = rr.Points2D(
            points,
            radii=radii,
            labels=labels,
            draw_order=draw_order,
            class_ids=class_ids,
            keypoint_ids=keypoint_ids,
        )
        print(f"{arch}\n")

        assert arch.points == rrc.Point2DArray.from_similar([[1.0, 2.0], [3.0, 4.0]])
        assert arch.radii == rrc.RadiusArray.from_similar([42, 43] if radii is not None else [])
        assert arch.labels == rrc.LabelArray.from_similar(["hello", "friend"] if labels is not None else [])
        assert arch.draw_order == rrc.DrawOrderArray.from_similar([300] if draw_order is not None else [])
        assert arch.class_ids == rrc.ClassIdArray.from_similar([126, 127] if class_ids is not None else [])
        assert arch.keypoint_ids == rrc.KeypointIdArray.from_similar([2, 3] if keypoint_ids is not None else [])


if __name__ == "__main__":
    test_points2d()
