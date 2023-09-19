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

    for image_from_cam, resolution in all_arrays:
        print(f"rr2.Pinhole(\n" f"image_from_cam={image_from_cam}\n" f"resolution={resolution}\n" f")")
        arch = rr2.Pinhole(image_from_cam=image_from_cam, resolution=resolution)
        print(f"{arch}\n")

        assert arch.image_from_cam == rrc.ImageFromCameraArray.optional_from_similar(
            [1, 2, 3, 4, 5, 6, 7, 8, 9] if image_from_cam is not None else None
        )
        assert arch.resolution == rrc.ResolutionArray.optional_from_similar([1, 2] if resolution is not None else None)


if __name__ == "__main__":
    test_pinhole()
