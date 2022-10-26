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

examples/objectron/main.py

examples/objectron/main.py --dir examples/objectron/dataset/camera/batch-5/31
```
"""


import argparse
import logging
import math
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Final, Iterable, Iterator, List

import numpy as np
import numpy.typing as npt
import rerun_sdk as rerun
from proto.objectron.proto import (
    ARCamera,
    ARFrame,
    ARPointCloud,
    FrameAnnotation,
    Object,
    ObjectType,
    Sequence,
)
from rerun_sdk import ImageFormat
from scipy.spatial.transform import Rotation as R

IMAGE_RESOLUTION: Final = (1440, 1920)
GEOMETRY_FILENAME: Final = "geometry.pbdata"
ANNOTATIONS_FILENAME: Final = "annotation.pbdata"


@dataclass
class SampleARFrame:
    """An `ARFrame` sample and the relevant associated metadata."""

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
    print(f"loading ARFrames from {path}")
    data = Path(path).read_bytes()

    frame_idx = 0
    while len(data) > 0 and frame_idx < nb_frames:
        next_len = int.from_bytes(data[:4], byteorder="little", signed=False)
        data = data[4:]

        frame = ARFrame().parse(data[:next_len])
        yield SampleARFrame(index=frame_idx, timestamp=frame.timestamp, dirpath=dirpath, frame=frame)

        data = data[next_len:]
        frame_idx += 1


def read_annotations(dirpath: Path) -> Sequence:
    """
    Loads the annotations from the given path on disk.

    `dirpath` should be of the form `dataset/bike/batch-8/16/`.
    """

    path = os.path.join(dirpath, ANNOTATIONS_FILENAME)
    print(f"loading annotations from {path}")
    data = Path(path).read_bytes()

    seq = Sequence().parse(data)

    return seq


def log_ar_frames(samples: Iterable[SampleARFrame], seq: Sequence) -> None:
    """Logs a stream of `ARFrame` samples and their annotations with the Rerun SDK."""

    rerun.log_world_coordinate_system("3d", up="+Y")

    frame_times = []
    for sample in samples:
        rerun.set_time_sequence("frame", sample.index)
        rerun.set_time_seconds("time", sample.timestamp)
        frame_times.append(sample.timestamp)

        img_path = Path(os.path.join(sample.dirpath, f"video/{sample.index}.jpg"))
        rerun.log_image_file("3d/camera/video", img_path, img_format=ImageFormat.JPEG)
        log_camera(sample.frame.camera)
        log_point_cloud(sample.frame.raw_feature_points)

    log_annotated_bboxes(seq.objects)
    log_frame_annotations(frame_times, seq.frame_annotations)


def log_camera(cam: ARCamera) -> None:
    """Logs a camera from an `ARFrame` using the Rerun SDK."""

    world_from_cam = np.asarray(cam.transform).reshape((4, 4))
    translation = world_from_cam[0:3, 3]
    intrinsics = np.asarray(cam.intrinsics).reshape((3, 3))
    rot = R.from_matrix(world_from_cam[0:3, 0:3])
    (w, h) = (cam.image_resolution_width, cam.image_resolution_height)

    # Because the dataset was collected in portrait:
    swizzle_x_y = np.asarray([[0, 1, 0], [1, 0, 0], [0, 0, 1]])
    intrinsics = swizzle_x_y @ intrinsics @ swizzle_x_y
    rot = rot * R.from_rotvec((math.tau / 4.0) * np.asarray([0.0, 0.0, 1.0]))
    (w, h) = (h, w)

    rerun.log_extrinsics(
        "3d/camera",
        rotation_q=rot.as_quat(),
        position=translation,
        camera_space_convention=rerun.CameraSpaceConvention.X_RIGHT_Y_UP_Z_BACK,
    )
    rerun.log_coordinate_system("3d/camera", "RUB") # X=Right, Y=Up, Z=Back
    rerun.log_intrinsics(
        "3d/camera/video",
        width=w,
        height=h,
        intrinsics_matrix=intrinsics,
    )


def log_point_cloud(point_cloud: ARPointCloud) -> None:
    """Logs a point cloud from an `ARFrame` using the Rerun SDK."""

    for i in range(point_cloud.count):
        point_raw = point_cloud.point[i]
        point = np.array([point_raw.x, point_raw.y, point_raw.z], dtype=np.float32)
        ident = point_cloud.identifier[i]
        rerun.log_point(f"3d/points/{ident}", point, color=[255, 255, 255, 255])


def log_annotated_bboxes(bboxes: Iterable[Object]) -> None:
    """Logs all the bounding boxes annotated in an `ARFrame` sequence using the Rerun SDK."""

    for bbox in bboxes:
        if bbox.type != ObjectType.BOUNDING_BOX:
            logging.error(f"err: object type not supported: {bbox.type}")
            continue

        rot = R.from_matrix(np.asarray(bbox.rotation).reshape((3, 3)))
        rerun.log_obb(
            f"3d/objects/{bbox.id}",
            bbox.scale,
            bbox.translation,
            rot.as_quat(),
            color=[130, 160, 250, 255],
            label=bbox.category,
            timeless=True,
        )


def log_frame_annotations(frame_times: List[float], frame_annotations: List[FrameAnnotation]) -> None:
    """Maps annotations to their associated `ARFrame` then logs them using the Rerun SDK."""

    for frame_ann in frame_annotations:
        frame_idx = frame_ann.frame_id
        if frame_idx >= len(frame_times):
            continue

        time = frame_times[frame_idx]
        rerun.set_time_sequence("frame", frame_idx)
        rerun.set_time_seconds("time", time)

        for obj_ann in frame_ann.annotations:
            keypoint_ids = [kp.id for kp in obj_ann.keypoints]
            keypoint_pos2s = np.asarray([[kp.point_2d.x, kp.point_2d.y] for kp in obj_ann.keypoints], dtype=np.float32)
            # NOTE: These are normalized points, so we need to bring them back to image space
            keypoint_pos2s *= IMAGE_RESOLUTION

            if len(keypoint_pos2s) == 9:
                log_projected_bbox(f"3d/camera/video/objects/{obj_ann.object_id}", keypoint_pos2s)
            else:
                for (id, pos2) in zip(keypoint_ids, keypoint_pos2s):
                    rerun.log_point(
                        f"3d/camera/video/objects/{obj_ann.object_id}/{id}",
                        pos2,
                        color=[130, 160, 250, 255],
                    )


def log_projected_bbox(path: str, keypoints: npt.NDArray[np.float32]) -> None:
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
    # fmt: off
    segments = np.array([keypoints[1], keypoints[2],
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
                         keypoints[4], keypoints[8]],
                         dtype=np.float32)
    # fmt: on

    rerun.log_line_segments(path, segments, color=[130, 160, 250, 255])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Logs Objectron data using the Rerun SDK.")
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument(
        "--frames", type=int, default=sys.maxsize, help="If specifies, limits the number of frames logged"
    )
    parser.add_argument(
        "--dir",
        type=Path,
        default="examples/objectron/dataset/camera/batch-5/31",
        help="Directories to log (e.g. `dataset/bike/batch-8/16/`)",
    )
    args = parser.parse_args()

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    samples = read_ar_frames(args.dir, args.frames)
    seq = read_annotations(args.dir)
    log_ar_frames(samples, seq)

    if args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()
