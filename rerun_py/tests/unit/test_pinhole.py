from __future__ import annotations

import itertools
from typing import TYPE_CHECKING

import numpy as np
import rerun as rr
from rerun.components import PinholeProjectionBatch, ResolutionBatch, ViewCoordinatesBatch

if TYPE_CHECKING:
    from rerun.datatypes import Mat3x3Like, Vec2DLike, ViewCoordinatesLike


def test_pinhole() -> None:
    image_from_cameras: list[Mat3x3Like] = [
        [[1, 2, 3], [4, 5, 6], [7, 8, 9]],
        [1, 2, 3, 4, 5, 6, 7, 8, 9],
        np.array([[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
    ]
    resolutions: list[Vec2DLike] = [[1, 2], (1, 2), np.array([1, 2])]
    camera_xyzs: list[ViewCoordinatesLike | None] = [
        None,
        rr.archetypes.ViewCoordinates.RDF,
        rr.components.ViewCoordinates.RDF,
        [3, 2, 5],
    ]

    all_arrays = itertools.zip_longest(image_from_cameras, resolutions, camera_xyzs)

    for image_from_camera, resolution, camera_xyz in all_arrays:
        image_from_camera = image_from_camera if image_from_camera is not None else image_from_cameras[-1]

        print(
            f"rr.Pinhole(\n"
            f"    image_from_camera={image_from_camera!s}\n"
            f"    resolution={resolution!s}\n"
            f"    camera_xyz={camera_xyz!s}\n"
            f")",
        )
        arch = rr.Pinhole(image_from_camera=image_from_camera, resolution=resolution, camera_xyz=camera_xyz)
        print(f"{arch}\n")

        assert arch.image_from_camera == PinholeProjectionBatch._converter([1, 2, 3, 4, 5, 6, 7, 8, 9])
        assert arch.resolution == ResolutionBatch._converter([1, 2] if resolution is not None else None)
        assert arch.camera_xyz == ViewCoordinatesBatch._converter(
            rr.components.ViewCoordinates.RDF if camera_xyz is not None else None,
        )


if __name__ == "__main__":
    test_pinhole()
