from __future__ import annotations

import itertools

import numpy as np
import rerun.experimental as rr2
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd


def test_pinhole() -> None:
    image_from_cameras: list[rrd.Mat3x3Like] = [
        [[1, 2, 3], [4, 5, 6], [7, 8, 9]],
        [1, 2, 3, 4, 5, 6, 7, 8, 9],
        np.array([[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
    ]
    resolutions: list[rrd.Vec2DLike] = [[1, 2], (1, 2), np.array([1, 2])]

    all_arrays = itertools.zip_longest(
        image_from_cameras,
        resolutions,
    )

    for image_from_camera, resolution in all_arrays:
        image_from_camera = image_from_camera if image_from_camera is not None else image_from_cameras[-1]

        print(
            f"rr2.Pinhole(\n"
            f"    image_from_camera={str(image_from_camera)}\n"
            f"    resolution={str(resolution)}\n"
            f")"
        )
        arch = rr2.Pinhole(image_from_camera=image_from_camera, resolution=resolution)
        print(f"{arch}\n")

        assert arch.image_from_camera == rrc.PinholeProjectionBatch._optional([1, 2, 3, 4, 5, 6, 7, 8, 9])
        assert arch.resolution == rrc.ResolutionBatch._optional([1, 2] if resolution is not None else None)


if __name__ == "__main__":
    test_pinhole()
