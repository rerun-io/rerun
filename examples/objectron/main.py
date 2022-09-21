#!/usr/bin/env python3

"""
Example of using the Rerun SDK to log the Objectron dataset.


Setup:
```sh
(cd examples/objectron && ./setup.sh)
```

Run:
```sh
# assuming your virtual env is up
python3 examples/objectron/main.py examples/objectron/dataset/bike/batch-8/16/
```
"""


import argparse
import logging
import math
import os
import sys

import numpy as np
import rerun_sdk as rerun

from dataclasses import dataclass
from pathlib import Path
from typing import List, Final, Iterator, Iterable

from PIL import Image
from scipy.spatial.transform import Rotation as R
from proto.objectron.proto import \
    ARFrame, ARCamera, ARPointCloud, Sequence, Object, ObjectType, FrameAnnotation


IMAGE_RESOLUTION: Final = (1440, 1920)
GEOMETRY_FILENAME: Final = "geometry.pbdata"
ANNOTATIONS_FILENAME: Final = "annotation.pbdata"


@dataclass
class SampleARFrame:
    """An `ARFrame` sample and associated relevant metadata."""
    index: int
    timestamp: float
    dirpath: Path
    frame: ARFrame


def read_ar_frames(dirpath: Path, nb_frames: int) -> Iterator[SampleARFrame]:
    """
    Loads up to `nb_frames` consecutive ARFrames from the given path on disk.

    `dirpath` should be of the form `dataset/bike/batch-8/16/`.
    """

    path = os.path.join(dirpath, GEOMETRY_FILENAME)
    print(f"loading ARFrame: {path}")
    data = Path(path).read_bytes()

    frame_idx = 0
    while len(data) > 0 and frame_idx < nb_frames:
        next_len = int.from_bytes(data[:4], byteorder='little', signed=False)
        data = data[4:]

        frame = ARFrame().parse(data[:next_len])

        yield SampleARFrame(index=frame_idx,
                            timestamp=frame.timestamp,
                            dirpath=dirpath,
                            frame=frame)

        data = data[next_len:]
        frame_idx += 1


def read_annotations(dirpath: Path) -> Sequence:
    """
    Loads the annotations from the given path on disk.

    `dirpath` should be of the form `dataset/bike/batch-8/16/`.
    """

    path = os.path.join(dirpath, ANNOTATIONS_FILENAME)
    print(f"loading annotations: {path}")
    data = Path(path).read_bytes()

    seq = Sequence().parse(data)

    return seq


def log_ar_frames(samples: Iterable[SampleARFrame], seq: Sequence):
    """Logs a stream of `ARFrame` samples and their annotations with the Rerun SDK."""

    rerun.set_space_up("world", [0, 1, 0])

    frame_times = []
    for sample in samples:
        rerun.set_time_sequence("frame", sample.index)
        rerun.set_time_seconds("time", sample.timestamp)
        frame_times.append(sample.timestamp)

        log_image(os.path.join(sample.dirpath, f"video/{sample.index}.jpg"))
        log_camera(sample.frame.camera)
        log_point_cloud(sample.frame.raw_feature_points)

    log_objects(seq.objects)
    log_frame_annotations(frame_times, seq.frame_annotations)


def log_image(path: str):
    # TODO(cmc): implement support for `log_img_file`
    print(f"logging image: {path}")

    img = Image.open(path)
    assert img.mode == 'RGB'

    rerun.log_image("video", np.asarray(img), space="image")


def log_camera(cam: ARCamera):
    world_from_cam = np.asarray(cam.transform).reshape((4, 4)).T
    translation = world_from_cam[3][:3]
    intrinsics = np.asarray(cam.intrinsics).reshape((3, 3))
    (w, h) = (cam.image_resolution_width, cam.image_resolution_height)

    rot = R.from_matrix(world_from_cam[0:3,][..., 0:3])
    # TODO(cmc): not sure why its last component is incorrectly negated?
    rot = R.from_quat(rot.as_quat() * [1.0, 1.0, 1.0, -1.0])

    # Because the dataset was collected in portrait:
    swizzle_x_y = np.asarray([[0, 1, 0], [1, 0, 0], [0, 0, 1]])
    intrinsics = swizzle_x_y.dot(intrinsics.dot(swizzle_x_y))
    axis = R.from_rotvec((math.tau / 4.0) * np.asarray([0.0, 0.0, 1.0]))
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

    # TODO(cmc): that would be ideal, but specifying individual paths within
    # batches is not supported for now.
    #
    # points = [[p.x, p.y, p.z] for p in point_cloud.point]
    # rerun.log_points("points",
    #                  points,
    #                  colors=np.asarray([255, 255, 255, 255]),
    #                  space="world")

    for i in range(count):
        point = point_cloud.point[i]
        ident = point_cloud.identifier[i]
        path = f"points/{ident}"

        rerun.log_points(path,
                         [[point.x, point.y, point.z]],
                         colors=np.asarray([255, 255, 255, 255]),
                         space="world")


def log_annotations(seq: Sequence):
    """Logs a sequence of annotations with the Rerun SDK."""

    rerun.set_space_up("world", [0, 1, 0])

    log_objects(seq.objects)
    log_frame_annotations(frame_times, seq.frame_annotations)


def log_objects(bboxes: Iterable[Object]):
    for bbox in bboxes:
        if bbox.type != ObjectType.BOUNDING_BOX:
            logging.error(f"err: object type not supported: {bbox.type}")
            continue

        # rot = R.from_matrix(np.asarray(obj.rotation).reshape((3, 3)))

        # rerun.log_obb(f"objects/{obj.id}/bbox3d",
        #               bbox.scale,
        #               bbox.translation,
        #               rot.as_quat(),
        #               timeless=True,
        #               color=[130, 160, 250, 255],
        #               space="world")


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
            keypoint_pos2s = np.asarray([[kp.point_2d.x, kp.point_2d.y]
                                         for kp in obj_ann.keypoints])
            # NOTE: These are normalized points, so we need to bring them back to image space
            keypoint_pos2s *= IMAGE_RESOLUTION

            if len(keypoint_pos2s) == 9:
                log_projected_bbox(f"{path}/bbox2d", keypoint_pos2s)
            else:
                # TODO(cmc): that would be ideal, but specifying individual paths within
                # batches is not supported for now.
                #
                # points = [[p.x, p.y, p.z] for p in point_cloud.point]
                # rerun.log_points("points",
                #                  keypoint_pos2s,
                #                  colors=np.asarray([130, 160, 250, 255]),
                #                  space="world")

                for (id, pos2) in zip(keypoint_ids, keypoint_pos2s):
                    path = f"{path}/bbox2d/{id}"
                    rerun.log_points(path,
                                     [pos2],
                                     colors=np.asarray([130, 160, 250, 255]),
                                     space="image")


def log_projected_bbox(path: str, keypoints: np.ndarray):
    """
    Projects the 3D bounding box described by the keypoints of an `ObjectAnnotation`
    to a 2D plane, using line segments.
    """

    # NOTE: we don't yet support projecting arbitrary 3D stuff onto 2D views, so
    # we manually render a 3D bounding box by drawing line segments using the
    # already projected coordinates.
    # Try commenting 2 out of the 3 blocks and running the whole thing again if
    # this doesn't make sense, that'll make everything clearer.
    #
    # TODO(cmc): replace once we can project 3D bboxes on 2D views
    segments = [keypoints[1], keypoints[2],
                keypoints[1], keypoints[3],
                keypoints[4], keypoints[2],
                keypoints[4], keypoints[3],

                keypoints[5], keypoints[6],
                keypoints[5], keypoints[7],
                keypoints[8], keypoints[6],
                keypoints[8], keypoints[7],

                keypoints[1], keypoints[5],
                keypoints[2], keypoints[6],
                keypoints[3], keypoints[7],
                keypoints[4], keypoints[8]]

    rerun.log_line_segments(path,
                            segments,
                            space="image",
                            color=[130, 160, 250, 255])


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
    parser.add_argument('--frames', type=int, default=sys.maxsize,
                        help='If specifies, limits the number of frames logged')
    parser.add_argument('dir', type=Path, nargs='+',
                        help='Directories to log (e.g. `dataset/bike/batch-8/16/`)')
    args = parser.parse_args()

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    for dirpath in args.dir:
        samples = read_ar_frames(dirpath, args.frames)
        seq = read_annotations(dirpath)
        log_ar_frames(samples, seq)

    if args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()
