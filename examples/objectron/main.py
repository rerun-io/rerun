#!/usr/bin/env python3

"""Example of using the Rerun SDK to log the Objectron dataset."""

import argparse
import math
import numpy as np
import os

from pathlib import Path
from typing import List

import rerun_sdk as rerun

from proto.objectron.proto import ARFrame, ARCamera, ARPointCloud, Sequence, Object, ObjectType, FrameAnnotation

## TODO: this is _extremely_ sloooooow
## TODO: how does all of this fits in John's work on CI, autofmt and stuff?

## ---

def relpath(path: Path):
    return os.path.join(os.path.abspath(os.path.dirname(__file__)), path)


def log_dir(dirpath: Path, nb_frames: int):
    rerun.set_space_up("world", [0, 1, 0])
    frame_times = log_geometry(dirpath, nb_frames)
    log_annotations(dirpath, frame_times)

## --- geometry ---

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
    intrinsics = np.asarray(cam.intrinsics).reshape((3, 3))
    (w, h) = (cam.image_resolution_width, cam.image_resolution_height)

    from scipy.spatial.transform import Rotation as R
    rot = R.from_matrix(world_from_cam[0:3,][..., 0:3])
    ## TODO: not sure why its last component is incorrectly negated?
    rot = R.from_quat(rot.as_quat() * [1.0, 1.0, 1.0, -1.0])

    ## Because the dataset was collected in portrait:
    swizzle_x_y = np.asarray([[0, 1, 0], [1, 0, 0], [0, 0, 1]])
    intrinsics = swizzle_x_y.dot(intrinsics.dot(swizzle_x_y))
    axis = R.from_rotvec((math.tau / 4.0) * np.array([0.0, 0.0, 1.0]))
    rot = rot * axis
    (w, h) = (h, w)

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

    ## TODO: that would be ideal, but labeling in batches is not supported for now
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


## --- annotations ---

def log_annotations(dirpath: Path, frame_times: List[float]):
    path = os.path.join(relpath(dirpath), 'annotation.pbdata')
    print(f"logging annotations: {path}")

    data = Path(path).read_bytes()
    seq = Sequence().parse(data)

    log_objects(seq.objects)
    log_frame_annotations(frame_times, seq.frame_annotations)


def log_objects(objects: List[Object]):
    for obj in objects:
        if obj.type != ObjectType.BOUNDING_BOX:
            ## TODO: error logging in python?
            print(f"err: object type not supported: {obj.type}")
            continue

        from scipy.spatial.transform import Rotation as R
        rot = R.from_matrix(np.asarray(obj.rotation).reshape((3, 3)))
        trans = np.asarray(obj.translation)
        half_size = np.asarray(obj.scale)

        ## TODO: implement support 3D bboxes first


def log_frame_annotations(frame_times: List[float], frame_annotations: List[FrameAnnotation]):
    for frame_ann in frame_annotations:
        frame_idx = frame_ann.frame_id
        if frame_idx >= len(frame_times):
            continue

        time = frame_times[frame_idx]
        rerun.set_time_sequence("frame", frame_idx)
        rerun.set_time_seconds("time", time)

        for obj_ann in frame_ann.annotations:
            path = f"objects/{obj_ann.object_id}"

            keypoint_ids = [kp.id for kp in obj_ann.keypoints]
            keypoint_pos2s = np.array([[kp.point_2d.x, kp.point_2d.y]
                                        for kp in obj_ann.keypoints])
            ## NOTE: These are normalized points, so we need to bring them back to image space
            keypoint_pos2s *= [1440.0, 1920.0]

            if len(keypoint_pos2s) == 9:
                path = f"{path}/bbox2d"
                ## NOTE: since we don't yet support projecting arbitrary 3D stuff onto 2D views,
                ## we manually draw a 3D bounding box by rendering line segmnents using the
                ## already projected coordinates.
                ## Try commenting 2 out of the 3 blocks if this doesn't make sense, that'll make
                ## everything clearer.
                ##
                ## TODO: replace with with native support for 3D-to-2D projection.
                segments = np.array([
                    keypoint_pos2s[1], keypoint_pos2s[2],
                    keypoint_pos2s[1], keypoint_pos2s[3],
                    keypoint_pos2s[4], keypoint_pos2s[2],
                    keypoint_pos2s[4], keypoint_pos2s[3],

                    keypoint_pos2s[5], keypoint_pos2s[6],
                    keypoint_pos2s[5], keypoint_pos2s[7],
                    keypoint_pos2s[8], keypoint_pos2s[6],
                    keypoint_pos2s[8], keypoint_pos2s[7],

                    keypoint_pos2s[1], keypoint_pos2s[5],
                    keypoint_pos2s[2], keypoint_pos2s[6],
                    keypoint_pos2s[3], keypoint_pos2s[7],
                    keypoint_pos2s[4], keypoint_pos2s[8],
                ])
                rerun.log_line_segments(path,
                                        segments,
                                        space="image",
                                        color=[130, 160, 250, 255])
            else:
                ## TODO: that would be ideal, but labeling in batches is not supported for now
                # points = np.array([[p.x, p.y, p.z] for p in point_cloud.point])
                # rerun.log_points("points",
                #                  keypoint_pos2s,
                #                  colors=np.array([130, 160, 250, 255]),
                #                  space="world")

                for (id, pos2) in zip(keypoint_ids, keypoint_pos2s):
                    path = f"{path}/bbox2d/{id}"
                    rerun.log_points(path,
                                     np.array([pos2]),
                                     colors=np.array([130, 160, 250, 255]),
                                     space="image")


## ---

if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Logs Objectron data using the Rerun SDK.')
    parser.add_argument('--headless', action='store_true',
                        help="Don't show GUI")
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
    elif args.headless:
        pass
    elif not args.connect:
        # Show the logged data inside the Python process:
        # TODO: this seem to crash when quitting?
        rerun.show()
