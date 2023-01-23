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
    # if args.connect:
    #    rr.connect(args.addr)

    rr.log_view_coordinates("world", up="-Y", timeless=True)

    for camera in cameras.values():
        # Log camera intrinsics
        rr.log_pinhole(
            f"world/camera{camera.id}/image",
            child_from_parent=intrinsics_for_camera(camera),
            width=camera.width,
            height=camera.height,
            timeless=True,
        )

    points_by_image = {id: [] for id in images.keys()}
    for point in points3D.values():
        for image_id in point.image_ids.tolist():
            points_by_image[image_id].append(point)

    for image in sorted(images.values(), key=lambda im: im.name):
        img_seq = int(image.name[3:7])
        rr.set_time_sequence("img_seq", img_seq)

        # COLMAP uses wxyz quaternions while Rerun uses xyzw
        quat_xyzw = image.qvec[[1, 2, 3, 0]]

        # Camera transform is "world to camera"
        rr.log_rigid3(
            f"world/camera{image.camera_id}",
            child_from_parent=(image.tvec, quat_xyzw),
            xyz="RDF",  # X=Right, Y=Down, Z=Forward
        )

        points = np.array([point.xyz for point in points_by_image[image.id]])
        point_colors = np.array([point.rgb for point in points_by_image[image.id]])
        rr.log_points(f"world/points", points, colors=point_colors)

        rr.log_image_file(f"world/camera{image.camera_id}/image/rgb", model_path.parent / "images" / image.name)
        rr.log_points(f"world/camera{image.camera_id}/image/keypoints", image.xys)


if __name__ == "__main__":
    main()
