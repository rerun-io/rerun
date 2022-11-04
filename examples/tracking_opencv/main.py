#!/usr/bin/env python3
"""Example applying simple object detection and tracking on a video."""
import json
import logging
import os
from pathlib import Path
from typing import Final

import cv2 as cv
import numpy as np
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

tracker = None
while cap.isOpened():
    ret, bgr = cap.read()
    rerun.set_time_sequence("frame", frame_idx)

    if not ret:
        print("End of video")
        break

    rgb = cv.cvtColor(bgr, cv.COLOR_BGR2RGB)
    rerun.log_image("image", rgb)

    if tracker is None or frame_idx % 40 == 0:
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

        # retrieve the ids corresponding to each mask

        rerun.log_rects(
            "image/downscaled/detections",
            boxes,
            rect_format=rerun.RectFormat.XYXY,
            labels=str_labels,
            colors=np.array(colors),
        )

        car_boxes_small = []
        for idx, label in enumerate(str_labels):
            if label == "car":
                car_boxes_small.append(boxes[idx, :])
        logging.info("Found %d cars", len(car_boxes_small))

        if car_boxes_small:
            top_car_bbox_small = car_boxes_small[0]

            x_min, y_min, x_max, y_max = top_car_bbox_small.tolist()
            bbox_small_xywh = [x_min, y_min, x_max - x_min, y_max - y_min]

            bbox_xywh = [int(val * DOWNSCALE_FACTOR) for val in bbox_small_xywh]
            logging.info("Inititlizing CSRT tracker")
            tracker = cv.TrackerCSRT_create()
            tracker.init(bgr, bbox_xywh)

            rerun.log_rect("image/tracked", bbox_xywh, rect_format=rerun.RectFormat.XYWH, label="car")
    else:
        logging.info("Running tracking update step for frame %d", frame_idx)
        success, bbox_xywh = tracker.update(bgr)

        if success:
            rerun.log_rect("image/tracked", bbox_xywh, rect_format=rerun.RectFormat.XYWH, label="car")
        else:
            logging.error("Tracking iteration failed for frame %d", frame_idx)
            tracker = None

    frame_idx += 1
