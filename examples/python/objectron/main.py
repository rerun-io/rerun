#!/usr/bin/env python3

"""
Example of using the Rerun SDK to log the Objectron dataset.

Example: `examples/python/objectron/main.py --recording chair`
"""

import argparse
import logging
import math
import os
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator, List

import numpy as np
import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk
from download_dataset import (
    ANNOTATIONS_FILENAME,
    AVAILABLE_RECORDINGS,
    GEOMETRY_FILENAME,
    IMAGE_RESOLUTION,
    LOCAL_DATASET_DIR,
    ensure_recording_available,
)
from proto.objectron.proto import (
    ARCamera,
    ARFrame,
    ARPointCloud,
    FrameAnnotation,
    Object,
    ObjectType,
    Sequence,
)
from scipy.spatial.transform import Rotation as R


@dataclass
class SampleARFrame:
    """An `ARFrame` sample and the relevant associated metadata."""

    index: int
    timestamp: float
    dirpath: Path
    frame: ARFrame
    image_path: Path


def read_ar_frames(
    dirpath: Path, num_frames: int, run_forever: bool, per_frame_sleep: float
) -> Iterator[SampleARFrame]:
    """
    Loads up to `num_frames` consecutive ARFrames from the given path on disk.

    `dirpath` should be of the form `dataset/bike/batch-8/16/`.
    """

    path = dirpath / GEOMETRY_FILENAME
    print(f"loading ARFrames from {path}")

    time_offset = 0
    frame_offset = 0

    while True:
        frame_idx = 0
        data = Path(path).read_bytes()
        while len(data) > 0 and frame_idx < num_frames:
            next_len = int.from_bytes(data[:4], byteorder="little", signed=False)
            data = data[4:]

            frame = ARFrame().parse(data[:next_len])
            img_path = Path(os.path.join(dirpath, f"video/{frame_idx}.jpg"))
            yield SampleARFrame(
                index=frame_idx + frame_offset,
                timestamp=frame.timestamp + time_offset,
                dirpath=dirpath,
                frame=frame,
                image_path=img_path,
            )

            data = data[next_len:]
            frame_idx += 1

            if run_forever and per_frame_sleep > 0.0:
                time.sleep(per_frame_sleep)

        if run_forever:
            time_offset += frame.timestamp
            frame_offset += frame_idx
        else:
            break


def read_annotations(dirpath: Path) -> Sequence:
    """
    Loads the annotations from the given path on disk.

    `dirpath` should be of the form `dataset/bike/batch-8/16/`.
    """

    path = dirpath / ANNOTATIONS_FILENAME
    print(f"loading annotations from {path}")
    data = Path(path).read_bytes()

    seq = Sequence().parse(data)

    return seq


def log_ar_frames(samples: Iterable[SampleARFrame], seq: Sequence) -> None:
    """Logs a stream of `ARFrame` samples and their annotations with the Rerun SDK."""

    rr.log_view_coordinates("world", up="+Y", timeless=True)

    log_annotated_bboxes(seq.objects)

    frame_times = []
    for sample in samples:
        rr.set_time_sequence("frame", sample.index)
        rr.set_time_seconds("time", sample.timestamp)
        frame_times.append(sample.timestamp)

        rr.log_image_file("world/camera/video", img_path=sample.image_path, img_format=rr.ImageFormat.JPEG)
        log_camera(sample.frame.camera)
        log_point_cloud(sample.frame.raw_feature_points)

    log_frame_annotations(frame_times, seq.frame_annotations)


def log_camera(cam: ARCamera) -> None:
    """Logs a camera from an `ARFrame` using the Rerun SDK."""

    X = np.asarray([1.0, 0.0, 0.0])
    Z = np.asarray([0.0, 0.0, 1.0])

    world_from_cam = np.asarray(cam.transform).reshape((4, 4))
    translation = world_from_cam[0:3, 3]
    intrinsics = np.asarray(cam.intrinsics).reshape((3, 3))
    rot = R.from_matrix(world_from_cam[0:3, 0:3])
    (w, h) = (cam.image_resolution_width, cam.image_resolution_height)

    # Because the dataset was collected in portrait:
    swizzle_x_y = np.asarray([[0, 1, 0], [1, 0, 0], [0, 0, 1]])
    intrinsics = swizzle_x_y @ intrinsics @ swizzle_x_y
    rot = rot * R.from_rotvec((math.tau / 4.0) * Z)
    (w, h) = (h, w)

    rot = rot * R.from_rotvec((math.tau / 2.0) * X)  # TODO(emilk): figure out why this is needed

    rr.log_transform3d(
        "world/camera",
        rr.TranslationRotationScale3D(translation, rr.Quaternion(xyzw=rot.as_quat())),
    )
    rr.log_view_coordinates("world/camera", xyz="RDF")  # X=Right, Y=Down, Z=Forward
    rr.log_pinhole(
        "world/camera/video",
        child_from_parent=intrinsics,
        width=w,
        height=h,
    )


def log_point_cloud(point_cloud: ARPointCloud) -> None:
    """Logs a point cloud from an `ARFrame` using the Rerun SDK."""

    positions = np.array([[p.x, p.y, p.z] for p in point_cloud.point]).astype(np.float32)
    identifiers = point_cloud.identifier
    rr.log_points("world/points", positions=positions, identifiers=identifiers, colors=[255, 255, 255, 255])


def log_annotated_bboxes(bboxes: Iterable[Object]) -> None:
    """Logs all the bounding boxes annotated in an `ARFrame` sequence using the Rerun SDK."""

    for bbox in bboxes:
        if bbox.type != ObjectType.BOUNDING_BOX:
            print(f"err: object type not supported: {bbox.type}")
            continue

        rot = R.from_matrix(np.asarray(bbox.rotation).reshape((3, 3)))
        rr.log_obb(
            f"world/annotations/box-{bbox.id}",
            half_size=0.5 * np.array(bbox.scale),
            position=bbox.translation,
            rotation_q=rot.as_quat(),
            color=[160, 230, 130, 255],
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
        rr.set_time_sequence("frame", frame_idx)
        rr.set_time_seconds("time", time)

        for obj_ann in frame_ann.annotations:
            keypoint_ids = [kp.id for kp in obj_ann.keypoints]
            keypoint_pos2s = np.asarray([[kp.point_2d.x, kp.point_2d.y] for kp in obj_ann.keypoints], dtype=np.float32)
            # NOTE: These are normalized points, so we need to bring them back to image space
            keypoint_pos2s *= IMAGE_RESOLUTION

            if len(keypoint_pos2s) == 9:
                log_projected_bbox(f"world/camera/video/estimates/box-{obj_ann.object_id}", keypoint_pos2s)
            else:
                for id, pos2 in zip(keypoint_ids, keypoint_pos2s):
                    rr.log_point(
                        f"world/camera/video/estimates/box-{obj_ann.object_id}/{id}",
                        pos2,
                        color=[130, 160, 250, 255],
                    )


def log_projected_bbox(path: str, keypoints: npt.NDArray[np.float32]) -> None:
    """
    Projects the 3D bounding box to a 2D plane, using line segments.

    The 3D bounding box is described by the keypoints of an `ObjectAnnotation`
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
                         keypoints[4], keypoints[8]], dtype=np.float32)
    # fmt: on

    rr.log_line_segments(path, segments, color=[130, 160, 250, 255])


def main() -> None:
    # Ensure the logging in download_dataset.py gets written to stderr:
    logging.getLogger().addHandler(logging.StreamHandler())
    logging.getLogger().setLevel("INFO")

    parser = argparse.ArgumentParser(description="Logs Objectron data using the Rerun SDK.")
    parser.add_argument(
        "--frames", type=int, default=sys.maxsize, help="If specified, limits the number of frames logged"
    )
    parser.add_argument("--run-forever", action="store_true", help="Run forever, continually logging data.")
    parser.add_argument(
        "--per-frame-sleep", type=float, default=0.1, help="Sleep this much for each frame read, if --run-forever"
    )
    parser.add_argument(
        "--recording",
        type=str,
        choices=AVAILABLE_RECORDINGS,
        default=AVAILABLE_RECORDINGS[1],
        help="The objectron recording to log to Rerun.",
    )
    parser.add_argument(
        "--force-reprocess-video",
        action="store_true",
        help="Reprocess video frames even if they already exist",
    )
    parser.add_argument(
        "--dataset_dir", type=Path, default=LOCAL_DATASET_DIR, help="Directory to save example videos to."
    )

    rr.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    rr.script_setup(args, "objectron")

    dir = ensure_recording_available(args.recording, args.dataset_dir, args.force_reprocess_video)

    samples = read_ar_frames(dir, args.frames, args.run_forever, args.per_frame_sleep)
    seq = read_annotations(dir)
    log_ar_frames(samples, seq)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
