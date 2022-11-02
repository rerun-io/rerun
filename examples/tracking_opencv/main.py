#!/usr/bin/env python3
"""Example applying simple object detection and tracking on a video."""
import json
import os
from pathlib import Path
from typing import Final

import cv2 as cv
import numpy as np
from PIL import Image
import torch
import rerun_sdk as rerun

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"
CACHE_DIR: Final = EXAMPLE_DIR / "cache"

# Comes from https://github.com/cocodataset/panopticapi/blob/master/panoptic_coco_categories.json
# License: https://github.com/cocodataset/panopticapi/blob/master/license.txt
COCO_CATEGORIES_PATH = EXAMPLE_DIR / "panoptic_coco_categories.json"

DOWNSCALE_FACTOR = 2

os.environ["TRANSFORMERS_CACHE"] = str(CACHE_DIR.absolute())
from transformers import DetrFeatureExtractor, DetrForSegmentation
from transformers.models.detr.feature_extraction_detr import rgb_to_id, masks_to_boxes


rerun.connect()

feature_extractor = DetrFeatureExtractor.from_pretrained("facebook/detr-resnet-50-panoptic")
model = DetrForSegmentation.from_pretrained("facebook/detr-resnet-50-panoptic")

with open(COCO_CATEGORIES_PATH) as f:
    categories = json.load(f)
id2Lable = {cat["id"]: cat["name"] for cat in categories}
id2IsThing = {cat["id"]: cat["isthing"] for cat in categories}
id2Color = {cat["id"]: cat["color"] for cat in categories}

car_id = model.config.label2id["car"]

video_path = str(DATASET_DIR / "a-car-drifting-in-asphalt-road-4569076-short.mp4")

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
        height, width, _ = rgb.shape
        small_size = (int(width / DOWNSCALE_FACTOR), int(height / DOWNSCALE_FACTOR))
        rgb_small = cv.resize(rgb, small_size)

        pil_im_smal = Image.fromarray(rgb_small)

        inputs = feature_extractor(images=pil_im_smal, return_tensors="pt")
        preprocessed = inputs["pixel_values"].detach().cpu().numpy()
        rerun.log_image("image/nn_preprocessed", rgb_small)
        rerun.log_unknown_transform("image/nn_preprocessed")  # Note: Haven't implemented 2D transforms yet.

        outputs = model(**inputs)

        # use the `post_process_panoptic` method of `DetrFeatureExtractor` to convert to COCO format
        processed_sizes = torch.as_tensor(tuple(reversed(small_size))).unsqueeze(0)
        result = feature_extractor.post_process_segmentation(outputs, processed_sizes)[0]

        scores = result["scores"].detach().cpu().numpy()
        masks = result["masks"].detach().cpu().numpy()
        labels = result["labels"].detach().cpu().numpy()
        str_labels = [id2Lable[l] for l in labels]
        colors = [id2Color[l] for l in labels]
        isThing = [id2IsThing[l] for l in labels]

        # retrieve the ids corresponding to each mask
        boxes = masks_to_boxes(masks)

        rerun.log_rects(
            "image/nn_preprocessed/detections",
            boxes,
            rect_format=rerun.RectFormat.XYXY,
            labels=str_labels,
            colors=np.array(colors),
        )

        num_cars = 0

        # if num_cars > 0:
        #    top_car_bbox_small = car_boxes_small[0, :]
        #
        #    x_min, y_min, x_max, y_max = top_car_bbox_small.tolist()
        #    bbox_small_xywh = [x_min, y_min, x_max - x_min, y_max - y_min]
        #
        #    bbox_xywh = [int(val * DOWNSCALE_FACTOR) for val in bbox_small_xywh]
        #    tracker = cv.TrackerCSRT_create()
        #    tracker.init(bgr, bbox_xywh)
        #
        #    rerun.log_rect("image/tracked", bbox_xywh, rect_format=rerun.RectFormat.XYWH, label="car")
    else:
        success, bbox_xywh = tracker.update(bgr)

        if success:
            rerun.log_rect("image/tracked", bbox_xywh, rect_format=rerun.RectFormat.XYWH, label="car")
        else:
            tracker = None

    frame_idx += 1
