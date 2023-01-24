#!/usr/bin/env python3
"""

"""

from pathlib import Path
import rerun as rr
import numpy as np
from argparse import ArgumentParser
from read_write_model import read_model, Camera


def intrinsics_for_camera(camera: Camera) -> np.array:
    """Convert a colmap camera to a pinhole camera intrinsics matrix."""
    return np.vstack(
        [
            np.hstack(
                [
                    # Focal length is in [:2]
                    np.diag(camera.params[:2]),
                    # Principle point is in [2:]
                    np.vstack(camera.params[2:]),
                ]
            ),
            [0, 0, 1],
        ]
    )


@rr.script("Visualize Colmap Data")
def main(parser: ArgumentParser) -> None:
    parser.add_argument("--input_model", help="path to input model folder")
    parser.add_argument("--input_format", choices=[".bin", ".txt"], help="input model format", default="")
    args = parser.parse_args()

    model_path = Path(args.input_model).expanduser()
    (cameras, images, points3D) = read_model(model_path, args.input_format)

    rr.init("colmap", spawn_and_connect=True)
    rr.log_view_coordinates("world", up="-Y", timeless=True)

    # Filter out noisy points
    filtered = {id: point for id, point in points3D.items() if point.rgb.any() and len(point.image_ids) > 4}

    for image in sorted(images.values(), key=lambda im: im.name):
        img_seq = int(image.name[0:4])
        quat_xyzw = image.qvec[[1, 2, 3, 0]]  # COLMAP uses wxyz quaternions
        camera_from_world = (image.tvec, quat_xyzw)
        camera = cameras[image.camera_id]
        intrinsics = intrinsics_for_camera(camera)

        visible_points = [filtered.get(id) for id in image.point3D_ids if id != -1]
        visible_points = [point for point in visible_points if point is not None]

        rr.set_time_sequence("img_seq", img_seq)

        points = [point.xyz for point in visible_points]
        point_colors = [point.rgb for point in visible_points]
        rr.log_points(f"world/points", points, colors=point_colors)

        # Camera transform is "world to camera"
        rr.log_rigid3(
            f"world/camera",
            child_from_parent=camera_from_world,
            xyz="RDF",  # X=Right, Y=Down, Z=Forward
        )

        # Log camera intrinsics
        rr.log_pinhole(
            f"world/camera/image",
            child_from_parent=intrinsics,
            width=camera.width,
            height=camera.height,
        )

        rr.log_image_file(f"world/camera/image/rgb", model_path.parent / "images" / image.name)

        rr.log_points(f"world/camera/image/keypoints", image.xys)


if __name__ == "__main__":
    main()
