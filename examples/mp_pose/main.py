"""TODO"""
import argparse
import logging
import os
from contextlib import closing
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Final, Generator, Iterator, List, Optional, Sequence

import cv2 as cv
import mediapipe as mp
import numpy as np
import numpy.typing as npt
import requests

import rerun as rr

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset" / "pose_movement"
DATASET_URL_BASE: Final = "https://storage.googleapis.com/rerun-example-datasets/pose_movement"


@dataclass
class VideoFrame:
    data: npt.NDArray[np.uint8]
    time: float
    idx: int


class VideoSource:
    def __init__(self, path: str):
        self.capture = cv.VideoCapture(path)

        if not self.capture.isOpened():
            logging.error("Couldn't open video at %s", path)

    def close(self) -> None:
        self.capture.release()

    def stream_bgr(self) -> Iterator[VideoFrame]:
        while self.capture.isOpened():
            idx = int(self.capture.get(cv.CAP_PROP_POS_FRAMES))
            is_open, bgr = self.capture.read()
            time_ms = self.capture.get(cv.CAP_PROP_POS_MSEC)

            if not is_open:
                break

            yield VideoFrame(data=bgr, time=time_ms * 1e-3, idx=idx)


def track_pose(video_path: str) -> None:
    mp_pose = mp.solutions.pose

    pose_points_desc = [rr.ClassDescription(id=l.value, label=l.value) for l in mp_pose.PoseLandmark]
    rr.log_annotation_context("/video/pose", pose_points_desc)

    with closing(VideoSource(video_path)) as video_source:
        with mp_pose.Pose() as pose:
            for bgr_frame in video_source.stream_bgr():

                rgb = cv.cvtColor(bgr_frame.data, cv.COLOR_BGR2RGB)
                rr.set_time_seconds("time", bgr_frame.time)
                rr.set_time_sequence("frame_idx", bgr_frame.idx)
                rr.log_image("video/rgb", rgb)

                results = pose.process(rgb)

                print("bla")


def get_downloaded_path(dataset_dir: Path, video_name: str) -> str:
    video_file_name = f"{video_name}.mp4"
    destination_path = dataset_dir / video_file_name
    if destination_path.exists():
        logging.info("%s already exists. No need to download", destination_path)
        return str(destination_path)

    source_path = f"{DATASET_URL_BASE}/{video_file_name}"

    logging.info("Downloading video from %s to %s", source_path, destination_path)
    os.makedirs(dataset_dir.absolute(), exist_ok=True)
    with requests.get(source_path, stream=True) as req:
        req.raise_for_status()
        with open(destination_path, "wb") as f:
            for chunk in req.iter_content(chunk_size=8192):
                f.write(chunk)
    return str(destination_path)


def setup_logging() -> None:
    logger = logging.getLogger()
    rerun_handler = rr.LoggingHandler("logs")
    rerun_handler.setLevel(-1)
    logger.addHandler(rerun_handler)
    stream_handler = logging.StreamHandler()
    stream_handler.setLevel(1)
    logger.addHandler(stream_handler)
    logger.setLevel(-1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Uses the MediaPipe Pose solution to track a human pose in video.")
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument(
        "--video",
        type=str,
        default="backflip",
        choices=["backflip", "soccer"],
        help="The example video to run on.",
    )
    parser.add_argument("--dataset_dir", type=Path, default=DATASET_DIR, help="Directory to save example videos to.")
    parser.add_argument("--video_path", type=str, default="", help="Full path to video to run on. Overrides `--video`.")

    args = parser.parse_args()
    setup_logging()

    video_path = args.video_path  # type: str
    if not video_path:
        video_path = get_downloaded_path(args.dataset_dir, args.video)

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rr.connect(args.addr)

    track_pose(video_path)

    if args.save is not None:
        rr.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rr.show()


if __name__ == "__main__":
    main()
