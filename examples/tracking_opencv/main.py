#!/usr/bin/env python3
"""Example applying simple object detection and tracking on a video."""
from dataclasses import dataclass
import json
import logging
import os
from pathlib import Path
from typing import Any, Final, List, Sequence

import cv2 as cv
import numpy as np
import numpy.typing as npt
from PIL import Image
import rerun_sdk as rerun

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"
CACHE_DIR: Final = EXAMPLE_DIR / "cache"

# Comes from https://github.com/cocodataset/panopticapi/blob/master/panoptic_coco_categories.json
# License: https://github.com/cocodataset/panopticapi/blob/master/license.txt
COCO_CATEGORIES_PATH = EXAMPLE_DIR / "panoptic_coco_categories.json"

DOWNSCALE_FACTOR = 2
DETECTION_SCORE_THRESHOLD = 0.8

os.environ["TRANSFORMERS_CACHE"] = str(CACHE_DIR.absolute())
from transformers import DetrFeatureExtractor, DetrForSegmentation
from transformers.models.detr.feature_extraction_detr import rgb_to_id, masks_to_boxes


@dataclass
class Detection:
    """Information about a detected object."""

    label_id: int
    label_str: str

    bbox_xywh: List[float]
    image_width: int
    image_height: int

    def scaled_to_fit(self, target_image: npt.NDArray[Any]) -> "Detection":
        """Rescales detection to fit to target image."""
        target_height, target_width = target_image.shape[:2]
        width_scale = target_width / self.image_width
        height_scale = target_height / self.image_height
        target_bbox = [
            self.bbox_xywh[0] * width_scale,
            self.bbox_xywh[1] * height_scale,
            self.bbox_xywh[2] * width_scale,
            self.bbox_xywh[3] * height_scale,
        ]
        return Detection(self.label_id, self.label_str, target_bbox, target_width, target_height)


class Tracker:
    next_tracking_id = 0

    def __init__(self, tracking_id: int, detection: Detection, bgr: npt.NDArray[np.uint8]) -> None:
        self.tracking_id = tracking_id
        self.tracked = detection.scaled_to_fit(target_image=bgr)

        self.tracker = cv.TrackerCSRT_create()
        bbox_xywh_rounded = [int(val) for val in self.tracked.bbox_xywh]
        self.tracker.init(bgr, bbox_xywh_rounded)
        self.log_tracked()

    @classmethod
    def track_new(cls, detection: Detection, bgr: npt.NDArray[np.uint8]) -> "Tracker":
        new_tracker = cls(cls.next_tracking_id, detection, bgr)
        cls.next_tracking_id += 1
        logging.info(
            "Tracking newly detected %s with tracking id #%d", new_tracker.tracked.label_str, new_tracker.tracking_id
        )
        return new_tracker

    def update(self, bgr: npt.NDArray[np.uint8]) -> None:
        success, bbox_xywh = self.tracker.update(bgr)

        if success:
            self.tracked.bbox_xywh = bbox_xywh
        else:
            logging.info("Tracker update failed for tracker with id #%d", self.tracking_id)
            self.tracker = None

        self.log_tracked()

    def log_tracked(self) -> None:
        if self.is_tracking:
            rerun.log_rect(
                f"image/tracked/{self.tracking_id}",
                self.tracked.bbox_xywh,
                rect_format=rerun.RectFormat.XYWH,
                label=self.tracked.label_str,
            )
        else:
            rerun.set_visible(
                f"image/tracked/{self.tracking_id}", False
            )  # TODO(Niko): Log this path as None instead once sdk can handle nullable rects

    def update_with_detection(self, detection: Detection, bgr: npt.NDArray[np.uint8]) -> None:
        self.tracked = detection.scaled_to_fit(target_image=bgr)
        self.tracker = cv.TrackerCSRT_create()
        bbox_xywh_rounded = [int(val) for val in self.tracked.bbox_xywh]
        self.tracker.init(bgr, bbox_xywh_rounded)
        self.log_tracked()

    @property
    def is_tracking(self) -> bool:
        return self.tracker is not None

    def match_score(self, other: Detection) -> float:
        if self.tracked.label_id != other.label_id:
            return 0.0
        if not self.is_tracking:
            return 0.0

        tracked_bbox = self.tracked.bbox_xywh
        other_bbox = other.bbox_xywh

        # Calcualte Intersection over Union (IoU)
        tracked_area = tracked_bbox[2] * tracked_bbox[3]
        other_area = other_bbox[2] * other_bbox[3]  # TODO: Need to rescale detections to match this

        rerun.log_rect(
            f"match/{self.tracking_id}/{other.label_str}/tracked", rect=tracked_bbox, rect_format=rerun.RectFormat.XYWH
        )
        rerun.log_rect(
            f"match/{self.tracking_id}/{other.label_str}/other", rect=other_bbox, rect_format=rerun.RectFormat.XYWH
        )

        overlap_width = max(
            0.0,
            min(tracked_bbox[0] + tracked_bbox[2], other_bbox[0] + other_bbox[2]) - max(tracked_bbox[0], other_bbox[0]),
        )
        overlap_height = max(
            0.0,
            min(tracked_bbox[1] + tracked_bbox[3], other_bbox[1] + other_bbox[3]) - max(tracked_bbox[1], other_bbox[1]),
        )
        intersection_area = overlap_width * overlap_height
        union_area = tracked_area + other_area - intersection_area

        return intersection_area / union_area


def update_trackers_with_detections(
    trackers: List[Tracker], detections: Sequence[Detection], bgr: npt.NDArray[np.uint8]
) -> List[Tracker]:
    """Tries to match detections to existing trackers and updates the trackers if they match.
    Any detections that don't match existing trackers will generate new trackers.
    Returns the new set of trackers.
    """
    non_updated_trackers = [tracker for tracker in trackers]  # shallow copy
    updated_trackers = []  # type: List[Tracker]

    logging.debug("Updating %d trackers with %d new detections", len(trackers), len(detections))
    for detection in detections:
        top_match_score = 0.0
        if trackers:
            scores = [tracker.match_score(detection) for tracker in non_updated_trackers]
            best_match_idx = np.argmax(scores)
            top_match_score = scores[best_match_idx]
        if top_match_score > 0.0:
            best_tracker = non_updated_trackers.pop(best_match_idx)
            best_tracker.update_with_detection(detection, bgr)
            updated_trackers.append(best_tracker)
        else:
            updated_trackers.append(Tracker.track_new(detection, bgr))

    logging.debug("Updateing %d trackers without matching detections")
    for tracker in non_updated_trackers:
        tracker.update(bgr)
        if tracker.is_tracking:
            updated_trackers.append(tracker)

    logging.info("Tracking %d objects after updating with %d new detections", len(updated_trackers), len(detections))

    return updated_trackers


# --- Actual scripts

logging.getLogger().addHandler(rerun.LoggingHandler("logs"))
logging.getLogger().setLevel(-1)

rerun.connect()

logging.info("Initializing neural net for detection and segmentation.")
feature_extractor = DetrFeatureExtractor.from_pretrained("facebook/detr-resnet-50-panoptic")
model = DetrForSegmentation.from_pretrained("facebook/detr-resnet-50-panoptic")

with open(COCO_CATEGORIES_PATH) as f:
    coco_categories = json.load(f)
class_descriptions = [
    rerun.ClassDescription(id=cat["id"], color=cat["color"], label=cat["name"]) for cat in coco_categories
]
rerun.log_class_descriptions("coco_categories", class_descriptions, timeless=True)

id2Lable = {cat["id"]: cat["name"] for cat in coco_categories}
id2IsThing = {cat["id"]: cat["isthing"] for cat in coco_categories}
id2Color = {cat["id"]: cat["color"] for cat in coco_categories}

car_id = model.config.label2id["car"]

video_path = str(DATASET_DIR / "a-car-drifting-in-asphalt-road-4569076-short.mp4")

logging.info("Loading input video: %s", str(video_path))
cap = cv.VideoCapture(video_path)
frame_idx = 0

trackers = []  # type: List[Tracker]
while cap.isOpened():
    ret, bgr = cap.read()
    rerun.set_time_sequence("frame", frame_idx)

    if not ret:
        print("End of video")
        break

    rgb = cv.cvtColor(bgr, cv.COLOR_BGR2RGB)
    rerun.log_image("image", rgb)

    if not trackers or frame_idx % 40 == 0:
        logging.info("Looking for things to track on frame %d", frame_idx)

        height, width, _ = rgb.shape
        small_size = (int(width / DOWNSCALE_FACTOR), int(height / DOWNSCALE_FACTOR))
        logging.debug(
            "Downscale image from (width, height) = %s to %s before passing to network", (width, height), small_size
        )
        rgb_small = cv.resize(rgb, small_size)
        rerun.log_image("image/downscaled", rgb_small)
        rerun.log_unknown_transform("image/downscaled")  # Note: Haven't implemented 2D transforms yet.

        logging.debug("Pass image to detection network")
        pil_im_smal = Image.fromarray(rgb_small)
        inputs = feature_extractor(images=pil_im_smal, return_tensors="pt")
        preprocessed = inputs["pixel_values"].detach().cpu().numpy()
        outputs = model(**inputs)

        logging.debug("Extracting detections and segmentations from network output")
        processed_sizes = [tuple(reversed(small_size))]
        segmentation_mask = feature_extractor.post_process_semantic_segmentation(outputs, processed_sizes)[0]
        detections = feature_extractor.post_process_object_detection(
            outputs, threshold=0.8, target_sizes=processed_sizes
        )[0]

        mask = segmentation_mask.detach().cpu().numpy().astype(np.uint8)
        rerun.log_segmentation_image("image/downscaled/segmentation", mask, class_descriptions="coco_categories")

        boxes = detections["boxes"].detach().cpu().numpy()
        labels = detections["labels"].detach().cpu().numpy()
        str_labels = [id2Lable[l] for l in labels]
        colors = [id2Color[l] for l in labels]
        isThing = [id2IsThing[l] for l in labels]

        rerun.log_rects(
            "image/downscaled/detections",
            boxes,
            rect_format=rerun.RectFormat.XYXY,
            labels=str_labels,
            colors=np.array(colors),
        )

        detections = []  # List[Detections]
        for idx, (label_id, label_str, is_thing) in enumerate(zip(labels, str_labels, isThing)):
            if is_thing:
                x_min, y_min, x_max, y_max = boxes[idx, :]
                bbox_xywh = [x_min, y_min, x_max - x_min, y_max - y_min]
                detections.append(
                    Detection(
                        label_id=label_id,
                        label_str=label_str,
                        bbox_xywh=bbox_xywh,
                        image_width=small_size[0],
                        image_height=small_size[1],
                    )
                )

        trackers = update_trackers_with_detections(trackers, detections, bgr)

    else:
        logging.info("Running tracking update step for frame %d", frame_idx)
        for tracker in trackers:
            tracker.update(bgr)
        trackers = [tracker for tracker in trackers if tracker.is_tracking]

    frame_idx += 1
