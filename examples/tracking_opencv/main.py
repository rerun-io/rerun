#!/usr/bin/env python3
"""Example applying simple object detection and tracking on a video."""
import os
from pathlib import Path
from typing import Final

import cv2 as cv
import numpy as np
from PIL import Image
import torch
import rerun_sdk as rerun

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
CACHE_DIR: Final = Path(os.path.dirname(__file__)) / "cache"

DOWNSCALE_FACTOR = 2

os.environ["TRANSFORMERS_CACHE"] = str(CACHE_DIR.absolute())
from transformers import DetrFeatureExtractor, DetrForObjectDetection

rerun.connect()

feature_extractor = DetrFeatureExtractor.from_pretrained("facebook/detr-resnet-50")
model = DetrForObjectDetection.from_pretrained("facebook/detr-resnet-50")

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

        rerun.log_image("image/downscaled", rgb_small)
        rerun.log_unknown_transform("image/downscaled")  # Note: Haven't implemented 2D transforms yet.

        pil_im_smal = Image.fromarray(rgb_small)

        inputs = feature_extractor(images=pil_im_smal, return_tensors="pt")
        outputs = model(**inputs)

        target_sizes = torch.tensor([pil_im_smal.size[::-1]])
        results = feature_extractor.post_process_object_detection(outputs, target_sizes=target_sizes)[0]

        labels = results["labels"].detach().cpu().numpy()
        boxes_small = results["boxes"].detach().cpu().numpy()

        car_boxes_small = boxes_small[labels == car_id, :]
        num_cars = car_boxes_small.shape[0]
        labels = ["car" for _ in range(num_cars)]
        rerun.log_rects(
            "image/downscaled/detections", car_boxes_small, rect_format=rerun.RectFormat.XYXY, labels=labels
        )

        if num_cars > 0:
            top_car_bbox_small = car_boxes_small[0, :]

            x_min, y_min, x_max, y_max = top_car_bbox_small.tolist()
            bbox_small_xywh = [x_min, y_min, x_max - x_min, y_max - y_min]

            bbox_xywh = [int(val * DOWNSCALE_FACTOR) for val in bbox_small_xywh]
            tracker = cv.TrackerCSRT_create()
            tracker.init(bgr, bbox_xywh)

            rerun.log_rect("image/tracked", bbox_xywh, rect_format=rerun.RectFormat.XYWH, label="car")
    else:
        success, bbox_xywh = tracker.update(bgr)

        if success:
            rerun.log_rect("image/tracked", bbox_xywh, rect_format=rerun.RectFormat.XYWH, label="car")
        else:
            tracker = None

    frame_idx += 1
