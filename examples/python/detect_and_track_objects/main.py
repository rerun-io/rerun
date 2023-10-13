#!/usr/bin/env python3
"""Example applying simple object detection and tracking on a video."""
from __future__ import annotations

import argparse
import json
import logging
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Final, Sequence

import cv2
import numpy as np
import numpy.typing as npt
import requests
import rerun as rr  # pip install rerun-sdk
from PIL import Image

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset" / "tracking_sequences"
DATASET_URL_BASE: Final = "https://storage.googleapis.com/rerun-example-datasets/tracking_sequences"
CACHE_DIR: Final = EXAMPLE_DIR / "cache"

# panoptic_coco_categories.json comes from:
# https://github.com/cocodataset/panopticapi/blob/master/panoptic_coco_categories.json
# License: https://github.com/cocodataset/panopticapi/blob/master/license.txt
COCO_CATEGORIES_PATH = EXAMPLE_DIR / "panoptic_coco_categories.json"

DOWNSCALE_FACTOR = 2
DETECTION_SCORE_THRESHOLD = 0.8

os.environ["TRANSFORMERS_CACHE"] = str(CACHE_DIR.absolute())
from transformers import (  # noqa: E402 module level import not at top of file
    DetrFeatureExtractor,
    DetrForSegmentation,
)

DESCRIPTION = """
# Detect and Track Objects

This is a more elaborate example applying simple object detection and segmentation on a video using the Huggingface
`transformers` library. Tracking across frames is performed using [CSRT](https://arxiv.org/abs/1611.08461) from
OpenCV. The results are visualized using Rerun.

## How it was made
The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/detect_and_track_objects/main.py).

### Input Video
The input video is logged as a sequence of
[rr.Image objects](https://www.rerun.io/docs/reference/types/archetypes/image) to the
[image/rgb entity](recording://image/rgb). Since the detection and segmentation model operates on smaller images the
resized images are logged to the separate [image_scaled/rgb entity](recording://image_scaled/rgb). This allows us to
subsequently visualize the segmentation mask on top of the video.

### Segmentations
The [segmentation result](recording://image_scaled/segmentation) is logged through a combination of two archetypes.
The segmentation image itself is logged as an
[rr.SegmentationImage archetype](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image) and
contains the id for each pixel. It is logged to the [image_scaled/segmentation entity](recording://image_scaled/segmentation).

The color and label for each class is determined by the
[rr.AnnotationContext archetype](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) which is
logged to the root entity using `rr.log("/", ..., timeless=True` as it should apply to the whole sequence and all
entities that have a class id.

### Detections
The detections and tracked bounding boxes are visualized by logging the
[rr.Boxes2D archetype](https://www.rerun.io/docs/reference/types/archetypes/boxes2d) to Rerun.

The color and label of the bounding boxes is determined by their class id, relying on the same
[rr.AnnotationContext archetype](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) as the
segmentation images. This ensures that a bounding box and a segmentation image with the same class id will also have the
same color.

Note that it is also possible to log multiple annotation contexts should different colors and / or labels be desired.
The annotation context is resolved by seeking up the entity hierarchy.

### Text Log
Through the [rr.TextLog archetype] text at different importance level can be logged. Rerun integrates with the
[Python logging module](https://docs.python.org/3/library/logging.html). After an initial setup that is described on the
[rr.TextLog page](https://www.rerun.io/docs/reference/types/archetypes/text_log#textlogintegration), statements
such as `logging.info("...")`, `logging.debug("...")`, etc. will show up in the Rerun viewer. In the viewer you can
adjust the filter level and look at the messages time-synchronized with respect to other logged data.
""".strip()


@dataclass
class Detection:
    """Information about a detected object."""

    class_id: int
    bbox_xywh: list[float]
    image_width: int
    image_height: int

    def scaled_to_fit_image(self, target_image: npt.NDArray[Any]) -> Detection:
        """Rescales detection to fit to target image."""
        target_height, target_width = target_image.shape[:2]
        return self.scaled_to_fit_size(target_width=target_width, target_height=target_height)

    def scaled_to_fit_size(self, target_width: int, target_height: int) -> Detection:
        """Rescales detection to fit to target image with given size."""
        if target_height == self.image_height and target_width == self.image_width:
            return self
        width_scale = target_width / self.image_width
        height_scale = target_height / self.image_height
        target_bbox = [
            self.bbox_xywh[0] * width_scale,
            self.bbox_xywh[1] * height_scale,
            self.bbox_xywh[2] * width_scale,
            self.bbox_xywh[3] * height_scale,
        ]
        return Detection(self.class_id, target_bbox, target_width, target_height)


class Detector:
    """Detects objects to track."""

    def __init__(self, coco_categories: list[dict[str, Any]]) -> None:
        logging.info("Initializing neural net for detection and segmentation.")
        self.feature_extractor = DetrFeatureExtractor.from_pretrained("facebook/detr-resnet-50-panoptic")
        self.model = DetrForSegmentation.from_pretrained("facebook/detr-resnet-50-panoptic")

        self.is_thing_from_id: dict[int, bool] = {cat["id"]: bool(cat["isthing"]) for cat in coco_categories}

    def detect_objects_to_track(self, rgb: npt.NDArray[np.uint8], frame_idx: int) -> list[Detection]:
        logging.info("Looking for things to track on frame %d", frame_idx)

        logging.debug("Preprocess image for detection network")
        pil_im_small = Image.fromarray(rgb)
        inputs = self.feature_extractor(images=pil_im_small, return_tensors="pt")
        _, _, scaled_height, scaled_width = inputs["pixel_values"].shape
        scaled_size = (scaled_width, scaled_height)
        rgb_scaled = cv2.resize(rgb, scaled_size)
        rr.log("image_scaled/rgb", rr.Image(rgb_scaled).compress(jpeg_quality=85))

        logging.debug("Pass image to detection network")
        outputs = self.model(**inputs)

        logging.debug("Extracting detections and segmentations from network output")
        processed_sizes = [(scaled_height, scaled_width)]
        segmentation_mask = self.feature_extractor.post_process_semantic_segmentation(outputs, processed_sizes)[0]
        detections = self.feature_extractor.post_process_object_detection(
            outputs, threshold=0.8, target_sizes=processed_sizes
        )[0]

        mask = segmentation_mask.detach().cpu().numpy().astype(np.uint8)
        rr.log("image_scaled/segmentation", rr.SegmentationImage(mask))

        boxes = detections["boxes"].detach().cpu().numpy()
        class_ids = detections["labels"].detach().cpu().numpy()
        things = [self.is_thing_from_id[id] for id in class_ids]

        self.log_detections(boxes, class_ids, things)

        objects_to_track: list[Detection] = []
        for idx, (class_id, is_thing) in enumerate(zip(class_ids, things)):
            if is_thing:
                x_min, y_min, x_max, y_max = boxes[idx, :]
                bbox_xywh = [x_min, y_min, x_max - x_min, y_max - y_min]
                objects_to_track.append(
                    Detection(
                        class_id=class_id,
                        bbox_xywh=bbox_xywh,
                        image_width=scaled_width,
                        image_height=scaled_height,
                    )
                )

        return objects_to_track

    def log_detections(self, boxes: npt.NDArray[np.float32], class_ids: list[int], things: list[bool]) -> None:
        things_np = np.array(things)
        class_ids_np = np.array(class_ids, dtype=np.uint16)

        thing_boxes = boxes[things_np, :]
        thing_class_ids = class_ids_np[things_np]
        rr.log(
            "image_scaled/detections/things",
            rr.Boxes2D(
                array=thing_boxes,
                array_format=rr.Box2DFormat.XYXY,
                class_ids=thing_class_ids,
            ),
        )

        background_boxes = boxes[~things_np, :]
        background_class_ids = class_ids[~things_np]
        rr.log(
            "image_scaled/detections/background",
            rr.Boxes2D(
                array=background_boxes,
                array_format=rr.Box2DFormat.XYXY,
                class_ids=background_class_ids,
            ),
        )


class Tracker:
    """
    Each instance takes care of tracking a single object.

    The factory class method `create_new_tracker` is used to give unique tracking id's per instance.
    """

    next_tracking_id = 0
    MAX_TIMES_UNDETECTED = 2

    def __init__(self, tracking_id: int, detection: Detection, bgr: npt.NDArray[np.uint8]) -> None:
        self.tracking_id = tracking_id
        self.tracked = detection.scaled_to_fit_image(bgr)
        self.num_recent_undetected_frames = 0

        self.tracker = cv2.TrackerCSRT_create()
        bbox_xywh_rounded = [int(val) for val in self.tracked.bbox_xywh]
        self.tracker.init(bgr, bbox_xywh_rounded)
        self.log_tracked()

    @classmethod
    def create_new_tracker(cls, detection: Detection, bgr: npt.NDArray[np.uint8]) -> Tracker:
        new_tracker = cls(cls.next_tracking_id, detection, bgr)
        cls.next_tracking_id += 1
        return new_tracker

    def update(self, bgr: npt.NDArray[np.uint8]) -> None:
        if not self.is_tracking:
            return
        success, bbox_xywh = self.tracker.update(bgr)

        if success:
            self.tracked.bbox_xywh = clip_bbox_to_image(
                bbox_xywh=bbox_xywh, image_width=self.tracked.image_width, image_height=self.tracked.image_height
            )
        else:
            logging.info("Tracker update failed for tracker with id #%d", self.tracking_id)
            self.tracker = None

        self.log_tracked()

    def log_tracked(self) -> None:
        if self.is_tracking:
            rr.log(
                f"image/tracked/{self.tracking_id}",
                rr.Boxes2D(
                    array=self.tracked.bbox_xywh,
                    array_format=rr.Box2DFormat.XYWH,
                    class_ids=self.tracked.class_id,
                ),
            )
        else:
            rr.log(f"image/tracked/{self.tracking_id}", rr.Clear(recursive=False))  # TODO(#3381)

    def update_with_detection(self, detection: Detection, bgr: npt.NDArray[np.uint8]) -> None:
        self.num_recent_undetected_frames = 0
        self.tracked = detection.scaled_to_fit_image(bgr)
        self.tracker = cv2.TrackerCSRT_create()
        bbox_xywh_rounded = [int(val) for val in self.tracked.bbox_xywh]
        self.tracker.init(bgr, bbox_xywh_rounded)
        self.log_tracked()

    def set_not_detected_in_frame(self) -> None:
        self.num_recent_undetected_frames += 1

        if self.num_recent_undetected_frames >= Tracker.MAX_TIMES_UNDETECTED:
            logging.info(
                "Dropping tracker with id #%d after not being detected %d times",
                self.tracking_id,
                self.num_recent_undetected_frames,
            )
            self.tracker = None
            self.log_tracked()

    @property
    def is_tracking(self) -> bool:
        return self.tracker is not None

    def match_score(self, other: Detection) -> float:
        """Returns bbox IoU if classes match, otherwise 0."""
        if self.tracked.class_id != other.class_id:
            return 0.0
        if not self.is_tracking:
            return 0.0

        other = other.scaled_to_fit_size(target_width=self.tracked.image_width, target_height=self.tracked.image_height)
        tracked_bbox = self.tracked.bbox_xywh
        other_bbox = other.bbox_xywh

        return box_iou(tracked_bbox, other_bbox)


def box_iou(first: list[float], second: list[float]) -> float:
    """Calculate Intersection over Union (IoU) between two 2D rectangles in XYWH format."""
    left = max(first[0], second[0])
    right = min(first[0] + first[2], second[0] + second[2])
    top = min(first[1] + first[3], second[1] + second[3])
    bottom = max(first[1], second[1])

    overlap_width = max(0.0, right - left)
    overlap_height = max(0.0, top - bottom)
    intersection_area = overlap_width * overlap_height

    tracked_area = first[2] * first[3]
    other_area = second[2] * second[3]
    union_area = tracked_area + other_area - intersection_area

    return intersection_area / union_area


def clip_bbox_to_image(bbox_xywh: list[float], image_width: int, image_height: int) -> list[float]:
    x_min = max(0, bbox_xywh[0])
    y_min = max(0, bbox_xywh[1])
    x_max = min(image_width - 1, bbox_xywh[0] + bbox_xywh[2])
    y_max = min(image_height - 1, bbox_xywh[1] + bbox_xywh[3])

    return [x_min, y_min, x_max - x_min, y_max - y_min]


def update_trackers_with_detections(
    trackers: list[Tracker],
    detections: Sequence[Detection],
    label_strs: Sequence[str],
    bgr: npt.NDArray[np.uint8],
) -> list[Tracker]:
    """
    Tries to match detections to existing trackers and updates the trackers if they match.

    Any detections that don't match existing trackers will generate new trackers.
    Returns the new set of trackers.
    """
    non_updated_trackers = list(trackers)  # shallow copy
    updated_trackers: list[Tracker] = []

    logging.debug("Updating %d trackers with %d new detections", len(trackers), len(detections))
    for detection in detections:
        top_match_score = 0.0
        if non_updated_trackers:
            scores = [tracker.match_score(detection) for tracker in non_updated_trackers]
            best_match_idx = np.argmax(scores)
            top_match_score = scores[best_match_idx]
        if top_match_score > 0.0:
            best_tracker = non_updated_trackers.pop(best_match_idx)
            best_tracker.update_with_detection(detection, bgr)
            updated_trackers.append(best_tracker)
        else:
            updated_trackers.append(Tracker.create_new_tracker(detection, bgr))
            logging.info(
                "Tracking newly detected %s with tracking id #%d",
                label_strs[detection.class_id],
                Tracker.next_tracking_id,
            )

    logging.debug("Updating %d trackers without matching detections", len(non_updated_trackers))
    for tracker in non_updated_trackers:
        tracker.set_not_detected_in_frame()
        tracker.update(bgr)
        if tracker.is_tracking:
            updated_trackers.append(tracker)

    logging.info("Tracking %d objects after updating with %d new detections", len(updated_trackers), len(detections))

    return updated_trackers


def track_objects(video_path: str, *, max_frame_count: int | None) -> None:
    with open(COCO_CATEGORIES_PATH) as f:
        coco_categories = json.load(f)
    class_descriptions = [
        rr.AnnotationInfo(id=cat["id"], color=cat["color"], label=cat["name"]) for cat in coco_categories
    ]
    rr.log("/", rr.AnnotationContext(class_descriptions), timeless=True)

    detector = Detector(coco_categories=coco_categories)

    logging.info("Loading input video: %s", str(video_path))
    cap = cv2.VideoCapture(video_path)
    frame_idx = 0

    label_strs = [cat["name"] or str(cat["id"]) for cat in coco_categories]
    trackers: list[Tracker] = []
    while cap.isOpened():
        if max_frame_count is not None and frame_idx >= max_frame_count:
            break

        ret, bgr = cap.read()
        rr.set_time_sequence("frame", frame_idx)

        if not ret:
            logging.info("End of video")
            break

        rgb = cv2.cvtColor(bgr, cv2.COLOR_BGR2RGB)
        rr.log("image/rgb", rr.Image(rgb).compress(jpeg_quality=85))

        if not trackers or frame_idx % 40 == 0:
            detections = detector.detect_objects_to_track(rgb=rgb, frame_idx=frame_idx)
            trackers = update_trackers_with_detections(trackers, detections, label_strs, bgr)

        else:
            if frame_idx % 10 == 0:
                logging.debug("Running tracking update step for frame %d", frame_idx)
            for tracker in trackers:
                tracker.update(bgr)
            trackers = [tracker for tracker in trackers if tracker.is_tracking]

        frame_idx += 1


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


def main() -> None:
    # Ensure the logging gets written to stderr:
    logging.getLogger().addHandler(logging.StreamHandler())
    logging.getLogger().setLevel("DEBUG")

    parser = argparse.ArgumentParser(description="Example applying simple object detection and tracking on a video.")
    parser.add_argument(
        "--video",
        type=str,
        default="horses",
        choices=["horses, driving", "boats"],
        help="The example video to run on.",
    )
    parser.add_argument("--dataset_dir", type=Path, default=DATASET_DIR, help="Directory to save example videos to.")
    parser.add_argument("--video_path", type=str, default="", help="Full path to video to run on. Overrides `--video`.")
    parser.add_argument(
        "--max-frame",
        type=int,
        help="Stop after processing this many frames. If not specified, will run until interrupted.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_detect_and_track_objects")

    setup_logging()

    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), timeless=True)

    video_path: str = args.video_path
    if not video_path:
        video_path = get_downloaded_path(args.dataset_dir, args.video)

    track_objects(video_path, max_frame_count=args.max_frame)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
