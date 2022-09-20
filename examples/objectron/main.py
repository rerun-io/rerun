#!/usr/bin/env python3

"""Example of using the Rerun SDK to log the Objectron dataset."""

import argparse
import math
import numpy as np
import os

from pathlib import Path
from typing import List

import rerun_sdk as rerun

from proto.research.compvideo.arcapture import ARFrame, ARCamera, ARPointCloud

def relpath(path: Path):
    return os.path.join(os.path.abspath(os.path.dirname(__file__)), path)

def log_dir(dirpath: Path, nb_frames: int):
    rerun.set_space_up("world", [0, 1, 0])
    frame_times = log_geometry(dirpath, nb_frames)
    print(frame_times)

def log_geometry(dirpath: Path, nb_frames: int) -> List[float]:
    path = os.path.join(relpath(dirpath), 'geometry.pbdata')
    print(f"logging geometry: {path}")

    frame_idx = 0
    frame_times = []

    data = Path(path).read_bytes()

    while len(data) > 0 and frame_idx < nb_frames:
        next_len = int.from_bytes(data[:4], byteorder='little', signed=False)
        data = data[4:]

        frame = ARFrame().parse(data[:next_len])
        data = data[next_len:]

        rerun.set_time_sequence("frame", frame_idx)
        rerun.set_time_seconds("time", frame.timestamp)
        frame_times.append(frame.timestamp)

        log_image(os.path.join(dirpath, f"video/{frame_idx}.jpg"))
        log_camera(frame.camera)
        log_point_cloud(frame.raw_feature_points)

        frame_idx += 1

    return frame_times


def log_image(path: str):
    print(f"logging image: {path}")

    from PIL import Image
    img = Image.open(path)
    assert img.mode == 'RGB'

    rgb = np.asarray(img)
    rerun.log_image("video", rgb, space="image")


def log_camera(cam: ARCamera):
    world_from_cam = np.asarray(cam.transform).reshape((4, 4)).T
    translation = world_from_cam[3][:3]
    intrinsics = np.asarray(cam.intrinsics).reshape((3, 3)).T
    (w, h) = (cam.image_resolution_width, cam.image_resolution_height)

    from scipy.spatial.transform import Rotation as R
    rot = R.from_euler("YXZ", [
        cam.euler_angles.yaw,
        cam.euler_angles.pitch,
        cam.euler_angles.roll,
    ])
    # rot = R.from_matrix(np.resize(world_from_cam, (3, 3)))

    ## Because the dataset was collected in portrait:
    swizzle_x_y = np.asarray([[0, 1, 0], [1, 0, 0], [0, 0, 1]])
    intrinsics = swizzle_x_y.dot(intrinsics.dot(swizzle_x_y))
    axis = R.from_rotvec((math.tau / 4.0) * np.array([0.0, 0.0, -1.0]))
    rot = rot * axis
    (w, h) = (h, w)

    # print(rot.as_quat())
    # print(translation)
    # print(intrinsics)
    # print(w, h)

    ## TODO: something fishy going on with those quaternions
    rerun.log_camera("camera",
                     resolution=[w, h],
                     intrinsics=intrinsics,
                     rotation_q=rot.as_quat(),
                     position=translation,
                     camera_space_convention=rerun.CameraSpaceConvention.X_RIGHT_Y_UP_Z_BACK,
                     space="world",
                     target_space="image")


def log_point_cloud(point_cloud: ARPointCloud):
    count = point_cloud.count

    ## TODO: that would be ideal, but how does one specify their identifiers in that case?
    # points = np.array([[p.x, p.y, p.z] for p in point_cloud.point])
    # rerun.log_points("points",
    #                  points,
    #                  colors=np.array([255, 255, 255, 255]),
    #                  space="world")

    for i in range(count):
        point = point_cloud.point[i]
        ident = point_cloud.identifier[i]
        path = f"points/{ident}"

        rerun.log_points(path,
                         np.array([[point.x, point.y, point.z]]),
                         colors=np.array([255, 255, 255, 255]),
                         space="world")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Logs Objectron data using the Rerun SDK.')
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    parser.add_argument('--addr', type=str, default=None,
                        help='Connect to this ip:port')
    parser.add_argument('--save', type=str, default=None,
                        help='Save data to a .rrd file at this path')
    parser.add_argument('--frames', type=int, default=2**32-1,
                        help='If specifies, limits the number of frames logged')
    parser.add_argument('dir', type=Path, nargs='+',
                        help='Directories to log (e.g. `dataset/bike/batch-8/16/`)')
    args = parser.parse_args()

    for dirpath in args.dir:
        log_dir(dirpath, args.frames)

    if args.save is not None:
        rerun.save(args.save)
    elif not args.connect:
        # Show the logged data inside the Python process:
        # TODO: this seem to crash when quitting?
        rerun.show()
